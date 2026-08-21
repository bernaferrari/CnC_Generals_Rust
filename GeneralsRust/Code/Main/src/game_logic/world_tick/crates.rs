//! Host tick `impl GameLogic` — `crates`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// When an AI computer unit kills and a money crate is spawned nearby, notify.
    ///
    /// C++ CreateCrateDie: only PLAYER_COMPUTER killers get notifyCrate.

    /// C++ CreateCrateDie::onDie residual for host destruction processing.
    ///
    /// Uses victim template `create_crate_data` list + last_damage_source as killer.
    /// Spawns money crates and notifies computer killers.

    /// C++ SalvageCrateCollide::executeCrateBehavior residual.
    ///
    /// Priority: armor set → weapon set (chance) → level (chance) → money.
    /// Returns (kind label, money granted).

    /// C++ VeterancyCrateCollide::executeCrateBehavior residual (non-pilot AOE).
    ///
    /// Grants `levels` to picker and same-team allies within effect_range
    /// (0 range = picker only). Fail-closed: not full pilot goal-object gate.

    /// Heals all living objects owned by the picker player (C++ healAllObjects).

    /// C++ ShroudCrateCollide::executeCrateBehavior residual.
    ///
    /// Permanently reveals the map for the picker's player (PartitionManager
    /// revealMapForPlayer).

    /// C++ ScriptEngine::transferObjectName residual (host bridge).
    ///
    /// Moves a script-visible name from `from_id` to `to_id` via host name field
    /// + NamedObjectTracker when available.

    /// C++ `targetCanEject`: a target exposes any `EjectPilotDie` interface.
    ///
    /// Vehicle/hijack eligibility remains owned by the surrounding Hijacker
    /// flow.  This final predicate must not infer the interface from a USA
    /// vehicle basename, and OCL/death-filter support is deliberately not
    /// consulted: C++ asks only `getEjectPilotDieInterface()` here.
    pub fn vehicle_supports_hijacker_ride(&self, vehicle_id: ObjectId) -> bool {
        let Some(v) = self.objects.get(&vehicle_id) else {
            return false;
        };
        if !v.is_alive() || v.status.destroyed {
            return false;
        }
        v.thing
            .template
            .eject_pilot_die
            .as_ref()
            .is_some_and(|metadata| metadata.has_eject_pilot_die_interface())
    }

    /// C++ HijackerUpdate airborne exit residual: ThingFactory newObject
    /// (ParachuteName) + ContainModule::addToContain(hijacker).
    ///
    /// Host residual: spawn AmericaParachute, dock rider inside, apply
    /// AmericaParachute freefall/open residual on both container and rider.
    pub(in super::super) fn put_hijacker_in_airborne_parachute(
        &mut self,
        rider_id: ObjectId,
        eject_pos: glam::Vec3,
    ) {
        use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
        use crate::game_logic::{KindOf, ThingTemplate};

        // Ensure AmericaParachute template exists for residual spawn.
        // C++ KINDOF_PARACHUTE — host has no Parachute kind; Vehicle + max_transport=1
        // residual so ContainModule::addToContain bookkeeping can hold the rider.
        if !self.templates.contains_key(HIJACKER_PARACHUTE_NAME) {
            let mut chute_tpl = ThingTemplate::new(HIJACKER_PARACHUTE_NAME);
            chute_tpl.add_kind_of(KindOf::Vehicle).set_health(1.0);
            self.templates
                .insert(HIJACKER_PARACHUTE_NAME.to_string(), chute_tpl);
        }

        let rider_team = self
            .objects
            .get(&rider_id)
            .map(|o| o.team)
            .unwrap_or(crate::game_logic::Team::Neutral);

        let mut pos = eject_pos;
        // Keep elevated for freefall residual (C++ m_ejectPos may already be high).
        if pos.y < 50.0 {
            pos.y = 50.0;
        }

        let Some(chute_id) = self.create_object(HIJACKER_PARACHUTE_NAME, rider_team, pos) else {
            // Fail-closed: still parachute the rider without a container object.
            if let Some(r) = self.objects.get_mut(&rider_id) {
                r.set_position(pos);
                crate::game_logic::host_ground_height_log::record(rider_id, 0.0, false);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(rider_id, Some([pos.x, pos.y, pos.z]));
                    r.record_host_movement();
                }
                r.apply_eject_parachuting();
            }
            return;
        };

        // Container residual bookkeeping + parachute physics on chute.
        {
            if let Some(chute) = self.objects.get_mut(&chute_id) {
                // Force 1-slot AmericaParachute residual capacity.
                chute.max_transport = 1;
                chute.record_host_contain_capacity();
                if !chute.enter_transport(rider_id) {
                    // Fail-closed: force occupant list even if kind gate rejects.
                    if !chute.occupants.contains(&rider_id) {
                        chute.occupants.push(rider_id);
                    }
                }
                chute.apply_eject_parachuting();
                // Parachute is not selectable residual (C++ drawable on container).
                chute.set_status_unselectable(true);
                chute.set_status_no_collisions(true);
            }
        }

        // Rider: contained + parachuting residual (hidden inside chute).
        if let Some(r) = self.objects.get_mut(&rider_id) {
            r.set_contained_by(Some(chute_id));
            r.set_ai_state(crate::game_logic::AIState::Docked);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(rider_id, 12);
                // Docked
            }
            r.set_position(pos);
            crate::game_logic::host_ground_height_log::record(rider_id, 0.0, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(rider_id, Some([pos.x, pos.y, pos.z]));
                r.record_host_movement();
            }
            r.apply_eject_parachuting();
            // Still not selectable while in chute (partition restore already cleared
            // MASKED from vehicle ride; chute contain keeps soft-hide).
            r.set_status_unselectable(true);
            r.set_status_no_collisions(true);
            r.set_status_masked(true);
        }

        self.car_bomb.record_airborne_parachute_put();
        // Tag air path honesty shared with EjectPilotDie air OCL residual.
        self.usa_pilot.record_air_ejection();
    }

    /// C++ CommandButtonHuntUpdate::setCommandButton residual (scripts/AI).
    pub fn start_command_button_hunt(
        &mut self,
        unit_id: ObjectId,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
    ) -> bool {
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.start_command_button_hunt(mode, frame);
        self.command_button_hunt_reg.record_start();
        true
    }

    /// C++ CommandButtonHuntUpdate::update residual for enter modes.
    pub fn tick_command_button_hunt_updates(&mut self) {
        use crate::game_logic::host_command_button_hunt::{
            hunt_allows_kind, hunt_allows_team, HostCommandButtonHuntMode,
            COMMAND_BUTTON_HUNT_SCAN_RANGE,
        };

        let frame = self.frame;

        let busy: std::collections::HashSet<ObjectId> =
            self.pending_special_abilities.keys().copied().collect();
        let hunters: Vec<(ObjectId, HostCommandButtonHuntMode, Team, glam::Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                let h = o.command_button_hunt.as_ref()?;
                if !h.due(frame) {
                    return None;
                }
                // C++: quit if last command not from AI — host residual: only when Idle.
                if !matches!(o.ai_state, AIState::Idle) {
                    return None;
                }
                if busy.contains(id) {
                    return None;
                }
                Some((*id, h.mode, o.team, o.get_position()))
            })
            .collect();

        for (hunter_id, mode, hunter_team, hunter_pos) in hunters {
            self.command_button_hunt_reg.record_scan();
            if let Some(h) = self
                .objects
                .get_mut(&hunter_id)
                .and_then(|o| o.command_button_hunt.as_mut())
            {
                h.schedule_next(frame);
            }

            let mut best: Option<(ObjectId, f32)> = None;
            for (tid, t) in self.objects.iter() {
                if *tid == hunter_id || !t.is_alive() {
                    continue;
                }
                let same_team = t.team == hunter_team;
                let target_neutral = t.team == Team::Neutral;
                if !hunt_allows_team(mode, same_team, target_neutral) {
                    continue;
                }
                let is_veh = t.is_kind_of(KindOf::Vehicle);
                let is_str = t.is_kind_of(KindOf::Structure);
                let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
                if !hunt_allows_kind(mode, is_veh, is_str, is_air) {
                    continue;
                }
                // Hijack residual: cannot re-hijack.
                if matches!(mode, HostCommandButtonHuntMode::HijackVehicle) && t.is_hijacked() {
                    continue;
                }
                let d = hunter_pos.distance(t.get_position());
                if d > COMMAND_BUTTON_HUNT_SCAN_RANGE {
                    continue;
                }
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*tid, d));
                }
            }

            let Some((target_id, _)) = best else {
                continue;
            };

            let ability = match mode {
                HostCommandButtonHuntMode::HijackVehicle => {
                    PendingSpecialAbility::Hijack { target_id }
                }
                HostCommandButtonHuntMode::ConvertToCarBomb => {
                    PendingSpecialAbility::CarBomb { target_id }
                }
                HostCommandButtonHuntMode::SabotageBuilding => {
                    PendingSpecialAbility::Sabotage { target_id }
                }
            };
            self.queue_pending_special_ability(hunter_id, ability);
            // Walk/path toward target residual.
            if let Some(tp) = self.objects.get(&target_id).map(|t| t.get_position()) {
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.target = Some(target_id);
                    u.set_ai_state(AIState::SpecialAbility);
                }
                let _ = self.assign_unit_path(hunter_id, tp, &[]);
            }
            self.command_button_hunt_reg.record_target();
        }
    }

    /// C++ DeployStyleAIUpdate pack/unpack timer residual.
    pub fn tick_deploy_style_updates(&mut self) {
        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.deploy_style.is_some())
            .map(|(id, _)| *id)
            .collect();

        // Auto-acquisition retains a normal attack target during DEPLOY so it
        // can resume when ReadyToAttack.  If that victim disappears while the
        // unit is AI_BUSY, clear the pending attack here rather than allowing a
        // stale ObjectId to survive until some unrelated future fire pass.
        // Restrict this to actual attack states so capture/repair/etc. targets
        // remain outside DeployStyle's combat authority.
        let stale_pending_attacks: Vec<ObjectId> = ids
            .iter()
            .copied()
            .filter(|id| {
                let Some(obj) = self.objects.get(id) else {
                    return false;
                };
                if obj.ai_state != AIState::Attacking
                    || !obj
                        .deploy_style
                        .as_ref()
                        .is_some_and(|deploy| deploy.is_busy())
                {
                    return false;
                }
                let Some(target_id) = obj.target else {
                    return false;
                };
                !self
                    .objects
                    .get(&target_id)
                    .is_some_and(|target| target.is_alive())
            })
            .collect();
        for id in stale_pending_attacks {
            self.stop_attack_decision_aware(id);
        }

        // Complete a timer before evaluating this frame's attack request,
        // matching DeployStyleAIUpdate::update's frame-boundary order.
        for &id in &ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(ds) = obj.deploy_style.as_mut() else {
                continue;
            };
            let (ready_atk, ready_mv) = ds.tick(frame);
            if ready_atk {
                obj.set_deployed(true);
            }
            if ready_mv {
                obj.set_deployed(false);
            }
        }

        // C++ evaluates its pending attack independently of weapon reload:
        // an in-range current victim starts DEPLOY even when the shot itself
        // will not become ready until later.  This keeps player and mood
        // attacks approaching while distant, but starts the authored timer as
        // soon as their selected weapon is actually in range.
        let in_range_pending_attacks: Vec<ObjectId> = ids
            .iter()
            .copied()
            .filter(|id| {
                let Some(obj) = self.objects.get(id) else {
                    return false;
                };
                if obj.get_template().deploy_style_metadata.is_none()
                    || !matches!(obj.ai_state, AIState::Attacking | AIState::AttackingGround)
                {
                    return false;
                }
                let Some(slot) = obj.selected_weapon_slot() else {
                    return false;
                };
                match (obj.target, obj.target_location) {
                    (Some(target_id), _) => self.objects.get(&target_id).is_some_and(|target| {
                        target.is_alive() && obj.is_within_attack_range_for_slot(slot, target)
                    }),
                    (None, Some(target_location)) => {
                        obj.is_within_attack_range_pos_for_slot(slot, target_location)
                    }
                    (None, None) => false,
                }
            })
            .collect();
        for id in in_range_pending_attacks {
            let started = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                let Some(deploy) = obj.deploy_style.as_mut() else {
                    continue;
                };
                if deploy.begin_deploy(frame) {
                    obj.stop_moving();
                    obj.set_status_moving(false);
                    true
                } else {
                    false
                }
            };
            if started {
                self.deploy_style_reg.record_deploy();
            }
        }
    }

    /// Ensure a source-authored DeployStyle unit is unpacking/unpacked before
    /// fire. Callers must establish a live, in-range attack target before
    /// invoking this; `DeployStyleAIUpdate::update` only enters `DEPLOY` at
    /// that point, not merely because an attack order exists.
    ///
    /// Returns false while the exact parsed `DeployStyleAIUpdate` module is
    /// packing or unpacking. A metadata/runtime mismatch is fail-closed rather
    /// than granting an unverified weapon bypass.
    pub fn ensure_deploy_style_ready_to_fire(&mut self, id: ObjectId) -> bool {
        let frame = self.frame;
        let mut started = false;
        let mut blocked = false;
        let ready = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return true;
            };
            // A unit without the parsed module is not a DeployStyle unit. Do
            // not infer this from template names or KindOf flags.
            if obj.get_template().deploy_style_metadata.is_none() {
                // A stale runtime block without source metadata is also an
                // invalid restore. It must not become an implicit name-based
                // deploy policy or a free fire permission.
                if obj.deploy_style.is_some() {
                    blocked = true;
                    obj.set_status_firing_weapon(false);
                    false
                } else {
                    true
                }
            } else {
                let ready = if let Some(ds) = obj.deploy_style.as_mut() {
                    if ds.is_ready_to_attack() {
                        true
                    } else {
                        if ds.begin_deploy(frame) {
                            started = true;
                            obj.stop_moving();
                            obj.set_status_moving(false);
                        } else {
                            blocked = true;
                        }
                        false
                    }
                } else {
                    // Object construction/save restore must install the live
                    // state from the metadata. Missing it may not let a
                    // deploy-only turret fire while packed.
                    blocked = true;
                    false
                };
                if !ready {
                    // Nested AttackStateMachine may have entered its fire
                    // state already. C++ DeployStyle marks it AI_BUSY, so no
                    // actual firing condition may remain set during the timer.
                    obj.set_status_firing_weapon(false);
                }
                ready
            }
        };
        if started {
            self.deploy_style_reg.record_deploy();
        }
        if blocked {
            self.deploy_style_reg.record_blocked_fire();
        }
        ready
    }

    /// C++ AssaultTransportAIUpdate wounded-retrieve + healthy re-exit residual.
    pub fn tick_assault_transport_updates(&mut self) {
        use crate::game_logic::host_troop_crawler::{
            is_assault_member_healthy, is_assault_member_wounded,
        };

        let crawler_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_troop_crawler_style_container()
                    && o.assault_transport
                        .as_ref()
                        .map(|a| a.active)
                        .unwrap_or(false)
            })
            .map(|(id, _)| *id)
            .collect();

        for crawler_id in crawler_ids {
            let (target_raw, members) = {
                let Some(c) = self.objects.get(&crawler_id) else {
                    continue;
                };
                let Some(a) = c.assault_transport.as_ref() else {
                    continue;
                };
                (a.designated_target, a.member_ids.clone())
            };
            let Some(target_raw) = target_raw else {
                continue;
            };
            let target_id = ObjectId(target_raw);
            // Target dead → clear assault.
            let target_alive = self
                .objects
                .get(&target_id)
                .map(|t| t.is_alive())
                .unwrap_or(false);
            if !target_alive {
                if let Some(c) = self.objects.get_mut(&crawler_id) {
                    if let Some(a) = c.assault_transport.as_mut() {
                        a.clear();
                    }
                }
                continue;
            }

            let mut still_members: Vec<u32> = Vec::new();
            let crawler_pos = self
                .objects
                .get(&crawler_id)
                .map(|c| c.get_position())
                .unwrap_or(Vec3::ZERO);

            for mid_raw in members {
                let mid = ObjectId(mid_raw);
                let Some(member) = self.objects.get(&mid) else {
                    continue;
                };
                if !member.is_alive() {
                    continue;
                }
                still_members.push(mid_raw);
                let contained = member.contained_by == Some(crawler_id);
                let wounded =
                    is_assault_member_wounded(member.health.current, member.health.maximum);
                let healthy =
                    is_assault_member_healthy(member.health.current, member.health.maximum);

                if contained {
                    // Full health → re-exit and resume attack (C++ isMemberHealthy).
                    if healthy {
                        if let Some(c) = self.objects.get_mut(&crawler_id) {
                            c.remove_occupant(mid);
                        }
                        if let Some(unit) = self.objects.get_mut(&mid) {
                            unit.set_contained_by(None);
                            let offset = Vec3::new(6.0, 0.0, 0.0);
                            unit.set_position(crawler_pos + offset);
                        }
                        if self.apply_engagement_decision_aware(mid, target_id) {
                            self.troop_crawler.record_healthy_redeploy();
                        }
                    }
                    continue;
                }

                // Outside + wounded → re-enter for heal.
                if wounded {
                    // Instant residual enter (path AI deferred).
                    if let Some(c) = self.objects.get_mut(&crawler_id) {
                        if !c.occupants.contains(&mid) && c.can_contain() {
                            let _ = c.add_occupant(mid);
                        }
                    }
                    if let Some(unit) = self.objects.get_mut(&mid) {
                        unit.set_contained_by(Some(crawler_id));
                        unit.stop_moving();
                        unit.target = None;
                        unit.set_status_attacking(false);
                        unit.set_position(crawler_pos);
                    }
                    self.troop_crawler.record_wounded_retrieve();
                    continue;
                }

                // Outside + not wounded → keep attacking designated target.
                if let Some(unit) = self.objects.get(&mid) {
                    if unit.target != Some(target_id) {
                        let _ = self.apply_engagement_decision_aware(mid, target_id);
                    }
                }
            }

            if let Some(c) = self.objects.get_mut(&crawler_id) {
                if let Some(a) = c.assault_transport.as_mut() {
                    a.member_ids = still_members;
                    if a.member_ids.is_empty() && c.occupants.is_empty() {
                        a.clear();
                    }
                }
            }
        }
    }

    /// C++ UndeadBody + BattleBusSlowDeathBehavior first-life / empty-hulk residual.
    pub fn tick_battle_bus_slow_deaths(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_battle_bus::battle_bus_undeath_passenger_damage;

        let frame = self.frame;

        // Snapshot bus state without overlapping borrows.
        let mut bus_snapshots: Vec<(ObjectId, bool, Vec<ObjectId>, usize, f32)> = Vec::new();
        for (id, o) in self.objects.iter() {
            if !o.is_battle_bus_transport {
                continue;
            }
            let Some(body) = o.battle_bus_body.as_ref() else {
                continue;
            };
            bus_snapshots.push((
                *id,
                body.pending_passenger_damage,
                o.occupants.clone(),
                o.occupants.len(),
                o.get_position().z,
            ));
        }

        let mut passenger_hits: Vec<(ObjectId, f32)> = Vec::new();
        for (bus_id, pending, occupants, _count, _z) in &bus_snapshots {
            if !*pending {
                continue;
            }
            if let Some(bus) = self.objects.get_mut(bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.pending_passenger_damage = false;
                }
            }
            for pid in occupants {
                if let Some(p) = self.objects.get(pid) {
                    let dmg = battle_bus_undeath_passenger_damage(p.health.maximum.max(1.0));
                    passenger_hits.push((*pid, dmg));
                }
            }
            self.battle_bus.record_undeath_detonate();
        }

        for (pid, dmg) in passenger_hits {
            if let Some(p) = self.objects.get_mut(&pid) {
                if p.is_alive() {
                    let _ = p.take_damage_from_typed(dmg, None, DamageType::Explosive);
                }
            }
        }

        let mut empty_kills: Vec<ObjectId> = Vec::new();
        for (bus_id, _pending, _occ, passenger_count, z) in &bus_snapshots {
            let above = *z > 0.5;
            let Some(bus) = self.objects.get_mut(bus_id) else {
                continue;
            };
            let (_landed, empty_kill) =
                bus.tick_battle_bus_slow_death(frame, above, *passenger_count);
            if empty_kill {
                empty_kills.push(*bus_id);
            }
        }

        for bus_id in empty_kills {
            self.battle_bus.record_empty_hulk_destruction();
            if let Some(bus) = self.objects.get_mut(&bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.mark_real_death();
                }
                let hp = bus.health.current.max(1.0) + 1.0;
                let _ = bus.take_damage_from_typed(hp, None, DamageType::Unresistable);
            }
            if self
                .objects
                .get(&bus_id)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(false)
            {
                let _ = self.destroy_object(bus_id);
            }
        }
    }

    /// Tick HijackerUpdate residual for all in-vehicle hijackers.
    pub fn tick_hijacker_updates(&mut self) {
        let riders: Vec<(ObjectId, ObjectId)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.hijacker_in_vehicle)
            .filter_map(|(id, o)| o.hijack_vehicle_id.map(|vid| (*id, vid)))
            .collect();
        for (rider_id, vehicle_id) in riders {
            let vehicle_alive = self
                .objects
                .get(&vehicle_id)
                .map(|v| v.is_alive() && !v.status.destroyed)
                .unwrap_or(false);
            if !vehicle_alive {
                let (epos, air) = {
                    let r = self.objects.get(&rider_id);
                    (
                        r.and_then(|o| o.hijacker_eject_pos)
                            .or_else(|| r.map(|o| o.get_position()))
                            .unwrap_or(glam::Vec3::ZERO),
                        r.map(|o| o.hijacker_was_airborne).unwrap_or(false),
                    )
                };
                if let Some(r) = self.objects.get_mut(&rider_id) {
                    r.end_hijacker_in_vehicle(epos, air);
                }
                // C++ HijackerUpdate: ThePartitionManager->registerObject(obj).
                if let Some(r) = self.objects.get(&rider_id) {
                    let p = r.get_position();
                    let fp = super::collide_dispatch::host_object_footprint(r);
                    self.partition_manager
                        .register_object_geometry(rider_id.0, p.x, p.z, fp);
                }

                // C++ HijackerUpdate: if m_wasTargetAirborne → PutInContainer
                // AmericaParachute (m_parachuteName) at m_ejectPos.
                if air {
                    self.put_hijacker_in_airborne_parachute(rider_id, epos);
                }
                continue;
            }
            let (vpos, air, vlevel, vxp) = {
                let v = self.objects.get(&vehicle_id).unwrap();
                (
                    v.get_position(),
                    v.status.airborne_target || v.get_position().y > 5.0,
                    v.experience.level,
                    v.experience.current,
                )
            };
            // Sync vehicle veterancy MAX back onto vehicle too.
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                // Will re-read rider level after tick
                let _ = v;
            }
            if let Some(r) = self.objects.get_mut(&rider_id) {
                r.tick_hijacker_in_vehicle(vpos, air, vlevel, vxp);
            }
            // Apply MAX level to vehicle from rider after tick.
            let rlevel = self
                .objects
                .get(&rider_id)
                .map(|r| r.experience.level)
                .unwrap_or(vlevel);
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                use crate::game_logic::VeterancyLevel;
                let rank = |l: VeterancyLevel| -> u8 {
                    match l {
                        VeterancyLevel::Rookie => 0,
                        VeterancyLevel::Veteran => 1,
                        VeterancyLevel::Elite => 2,
                        VeterancyLevel::Heroic => 3,
                    }
                };
                if rank(rlevel) > rank(v.experience.level) {
                    let prev = v.experience.level;
                    v.experience.level = rlevel;
                    v.apply_veterancy_bonuses(prev, rlevel);
                }
            }
        }
    }
    pub fn transfer_script_object_name(&mut self, from_id: ObjectId, to_id: ObjectId) -> bool {
        use gamelogic::scripting::engine::get_named_object_tracker;
        let name = self
            .objects
            .get(&from_id)
            .map(|o| o.name.clone())
            .filter(|n| !n.is_empty());
        let Some(n) = name else {
            return false;
        };
        if let Some(t) = self.objects.get_mut(&to_id) {
            t.name = n.clone();
            t.record_host_identity();
        }
        if let Some(f) = self.objects.get_mut(&from_id) {
            f.name.clear();
        }
        // Register on tracker with host ObjectId (no dual-world engine id).
        let tracker_id = to_id.0;
        let tracker = get_named_object_tracker();
        let _ = tracker.register_named_object(n, tracker_id);
        true
    }
    pub fn execute_shroud_crate_behavior(&mut self, picker_id: ObjectId) -> bool {
        let team = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => p.team,
            _ => return false,
        };
        // Map host team → player id residual.
        let player_id = self
            .players
            .iter()
            .find(|(_, p)| p.team == team)
            .map(|(id, _)| *id)
            .unwrap_or(match team {
                Team::USA => 0,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            });
        self.partition_manager.reveal_map_for_player(player_id);
        true
    }
    pub fn execute_heal_crate_behavior(&mut self, picker_id: ObjectId) -> usize {
        let picker_owner = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => p.owner_player_id,
            _ => return 0,
        };
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.destroyed
                    && match picker_owner {
                        Some(pid) => o.owner_player_id == Some(pid),
                        None => false,
                    }
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if let Some(o) = self.objects.get_mut(&id) {
                let max = o.health.maximum;
                if o.health.current < max {
                    Self::write_object_health_authority_aware(o, max);
                    n += 1;
                }
            }
        }
        n
    }

    /// C++ UnitCrateCollide::executeCrateBehavior residual.
    ///
    /// Missing ThingTemplate → FALSE. Spawn on picker player default team
    /// via `findPositionAround` maxRadius 20.
    pub fn execute_unit_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        unit_type: &str,
        count: u32,
    ) -> usize {
        let (team, origin, ori) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position(), p.get_orientation()),
            _ => return 0,
        };
        if unit_type.is_empty() || count == 0 {
            return 0;
        }
        if !self.templates.contains_key(unit_type) {
            return 0;
        }
        let occupied: Vec<(glam::Vec3, f32)> = self
            .objects
            .values()
            .filter(|o| o.is_alive() && !o.status.destroyed)
            .map(|o| (o.get_position(), o.thing.geometry.radius.max(1.0)))
            .collect();
        let mut spawned = 0usize;
        for i in 0..count {
            let pos = crate::game_logic::host_supply_gather::find_position_around_xz(
                origin,
                &occupied,
                i,
                picker_id.0,
            );
            if let Some(nid) = self.create_object(unit_type, team, pos) {
                if let Some(o) = self.objects.get_mut(&nid) {
                    o.set_orientation(ori);
                }
                spawned += 1;
            }
        }
        spawned
    }

    pub fn execute_veterancy_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        effect_range: f32,
        levels: u8,
    ) -> usize {
        let (team, origin, picker_player) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position(), p.owner_player_id),
            _ => return 0,
        };
        // C++ getLevelsToGain: Regular + AddsOwnerVeterancy is 0 → crate invalid.
        if levels == 0 {
            return 0;
        }
        let range_sq = effect_range.max(0.0) * effect_range.max(0.0);
        let targets: Vec<ObjectId> = if effect_range <= 0.0 {
            vec![picker_id]
        } else {
            self.objects
                .iter()
                .filter(|(_, o)| {
                    o.team == team
                        && o.is_alive()
                        && !o.status.destroyed
                        && !o.status.under_construction
                        && !o.is_kind_of(KindOf::Structure)
                        && o.is_trainable()
                        && !matches!(o.experience.level, crate::game_logic::VeterancyLevel::Heroic)
                        && {
                            let p = o.get_position();
                            let dx = p.x - origin.x;
                            let dz = p.z - origin.z;
                            dx * dx + dz * dz <= range_sq
                        }
                        && picker_player
                            .map(|pid| o.owner_player_id == Some(pid))
                            .unwrap_or(true)
                })
                .map(|(id, _)| *id)
                .collect()
        };
        let mut n = 0usize;
        for tid in targets {
            if let Some(o) = self.objects.get_mut(&tid) {
                if !o.is_trainable() {
                    continue;
                }
                let g = o.gain_exp_for_level(levels, true);
                if g > 0 {
                    n += 1;
                }
            }
        }
        n
    }
    pub fn execute_salvage_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        money_provided: u32,
        seed: u32,
    ) -> (&'static str, u32) {
        use crate::game_logic::host_gamedata_lobby_residual::{
            SALVAGE_LEVEL_CHANCE_RESIDUAL, SALVAGE_WEAPON_CHANCE_RESIDUAL,
        };
        use crate::game_logic::host_rng_residual::pure_logic_random_real;
        use crate::game_logic::VeterancyLevel;

        let Some(picker) = self.objects.get_mut(&picker_id) else {
            return ("none", 0);
        };
        // Armor salvager path (no percent).
        if picker.is_kind_of(KindOf::ArmorSalvager) && picker.armor_crate_upgrade < 2 {
            picker.apply_salvage_armor_upgrade();
            return ("armor", 0);
        }
        // Weapon salvager path.
        if picker.is_kind_of(KindOf::WeaponSalvager) && picker.weapon_crate_upgrade < 2 {
            let roll = pure_logic_random_real(seed, 1, 0.0, 1.0);
            if SALVAGE_WEAPON_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_WEAPON_CHANCE_RESIDUAL
            {
                picker.apply_salvage_weapon_upgrade();
                return ("weapon", 0);
            }
        }
        // C++ SalvageCrateCollide::eligibleForLevel: not HEROIC and isTrainable.
        // Untrainable pickers fall through to doMoney instead of burning the crate.
        let can_level = picker.is_trainable()
            && !matches!(picker.experience.level, VeterancyLevel::Heroic);
        if can_level {
            let roll = pure_logic_random_real(seed, 2, 0.0, 1.0);
            if SALVAGE_LEVEL_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_LEVEL_CHANCE_RESIDUAL
            {
                picker.apply_salvage_level_gain();
                return ("level", 0);
            }
        }
        // Money fallback.
        let money = if money_provided > 0 {
            money_provided
        } else {
            crate::game_logic::host_create_crate_die::salvage_money_roll(seed, 3)
        };
        ("money", money)
    }
    pub fn try_create_crates_on_die(
        &mut self,
        victim_id: ObjectId,
        victim_pos: glam::Vec3,
        victim_team: Team,
        crate_data: &[String],
        killer_id: Option<ObjectId>,
    ) -> usize {
        if crate_data.is_empty() {
            return 0;
        }
        // Ally kill → no crate (C++ Relationship ALLIES).
        if let Some(kid) = killer_id {
            if let Some(k) = self.objects.get(&kid) {
                if k.team == victim_team {
                    return 0;
                }
            }
        }
        let victim_vet = self
            .objects
            .get(&victim_id)
            .map(|v| format!("{:?}", v.experience.level));
        let killer_kind_names: Vec<String> = killer_id
            .and_then(|kid| self.objects.get(&kid))
            .map(|k| {
                k.thing
                    .template
                    .kind_of
                    .iter()
                    .map(|ko| format!("{ko:?}"))
                    .collect()
            })
            .unwrap_or_default();
        let killer_kind_refs: Vec<&str> = killer_kind_names.iter().map(|s| s.as_str()).collect();
        let killer_sciences: Vec<String> = killer_id
            .and_then(|kid| self.objects.get(&kid).and_then(|k| k.owner_player_id))
            .and_then(|pid| {
                self.players
                    .get(&pid)
                    .map(|p| p.unlocked_sciences.iter().cloned().collect())
            })
            .unwrap_or_default();
        let victim_owner = self
            .objects
            .get(&victim_id)
            .and_then(|v| v.owner_player_id);
        let seed = crate::game_logic::host_create_crate_die::crate_die_seed(
            victim_id, killer_id, self.frame,
        );
        let mut spawned = 0usize;
        for (i, name) in crate_data.iter().enumerate() {
            let draw = (i as u32).wrapping_mul(7);
            let gates = crate::game_logic::host_create_crate_die::CrateDieGates {
                victim_veterancy: victim_vet.as_deref(),
                killer_kindof_names: &killer_kind_refs,
                killer_sciences: &killer_sciences,
            };
            let Some(req) = crate::game_logic::host_create_crate_die::try_roll_crate_spawn_gated(
                name, seed, draw, Some(&gates),
            ) else {
                continue;
            };

            // Spawn offset residual (fail-closed vs findPositionAround).
            let ang = (seed.wrapping_add(i as u32) as f32) * 0.7;
            let pos = glam::Vec3::new(
                victim_pos.x + ang.cos() * 5.0,
                victim_pos.y,
                victim_pos.z + ang.sin() * 5.0,
            );
            // Ensure template exists.
            if !self.templates.contains_key(&req.object_name) {
                let mut t = ThingTemplate::new(&req.object_name);
                t.add_kind_of(crate::game_logic::KindOf::Crate);
                // Crates are non-combat pickups.
                self.templates.insert(req.object_name.clone(), t);
            } else if let Some(existing) = self.templates.get_mut(&req.object_name) {
                existing.add_kind_of(crate::game_logic::KindOf::Crate);
            }
            let crate_team = if req.owned_by_maker {
                victim_team
            } else {
                Team::Neutral
            };
            let Some(crate_id) = self.create_object(&req.object_name, crate_team, pos) else {
                continue;
            };
            if req.owned_by_maker {
                if let Some(crate_obj) = self.objects.get_mut(&crate_id) {
                    crate_obj.owner_player_id = victim_owner;
                }
            }
            if let Some(crate_obj) = self.objects.get_mut(&crate_id) {
                crate_obj.apply_crate_terrain_decal();
            }

            if req.is_shroud_crate {
                self.host_money_crates.register_shroud_crate(crate_id);
            } else if req.is_heal_crate {
                self.host_money_crates.register_heal_crate(crate_id);
            } else if req.is_unit_crate {
                self.host_money_crates.register_unit_crate(
                    crate_id,
                    &req.unit_crate_type,
                    req.unit_crate_count,
                );
            } else if req.is_veterancy {
                self.host_money_crates.register_level_up_crate(
                    crate_id,
                    req.veterancy_effect_range,
                    req.veterancy_levels,
                );
            } else if req.object_name.eq_ignore_ascii_case("SalvageCrate") {
                self.host_money_crates
                    .register_salvage_crate(crate_id, req.money_provided);
            } else {
                self.host_money_crates.register(
                    crate_id,
                    req.money_provided,
                    req.building_pickup,
                    if req.building_pickup { 25 } else { 0 },
                );
            }
            self.host_money_crates
                .apply_crate_collide_gates(crate_id, &req.object_name);
            // C++ DeletionUpdate residual on crate object.
            self.host_money_crates.arm_default_deletion(
                crate_id,
                self.frame,
                crate_id.0.wrapping_add(self.frame),
            );
            if let Some(kid) = killer_id {
                let _ = self.notify_computer_killer_of_crate(kid, crate_id);
            }
            spawned += 1;
        }
        spawned
    }
    pub fn notify_computer_killer_of_crate(
        &mut self,
        killer_id: ObjectId,
        crate_id: ObjectId,
    ) -> bool {
        let killer_team = match self.objects.get(&killer_id) {
            Some(k) if k.is_alive() => k.team,
            _ => return false,
        };
        // Computer = non-local player residual.
        let is_computer = self
            .players
            .values()
            .find(|p| p.team == killer_team)
            .map(|p| !p.is_local)
            .unwrap_or(true); // no player record → treat as AI
        if !is_computer {
            return false;
        }
        self.notify_unit_crate(killer_id, crate_id)
    }
    pub fn try_idle_repulse(&mut self, unit_id: ObjectId) -> bool {
        if !self.enable_repulsors {
            return false;
        }
        let (vision, is_idle, can_be, alive, pos) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            if !u.is_kind_of(crate::game_logic::KindOf::CanBeRepulsed) {
                return false;
            }
            // C++ ai->isIdle()
            let idle = matches!(u.ai_state, crate::game_logic::AIState::Idle)
                && u.target.is_none()
                && u.move_away_from.is_none();
            if !idle {
                return false;
            }
            let vision = u.vision_range.max(50.0);
            (vision, idle, true, true, u.get_position())
        };
        let _ = (is_idle, can_be, alive, pos);
        let Some((rep_id, _)) = self.find_closest_repulsor(unit_id, vision) else {
            return false;
        };
        let rep_pos = match self.objects.get(&rep_id) {
            Some(r) => r.get_position(),
            None => return false,
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            // C++ AIMoveAwayFromRepulsorsState::onEnter (AIStates.cpp:2272-2276):
            // chooseLocomotorSet(LOCOMOTORSET_PANIC) + MODELCONDITION_PANICKING.
            crate::game_logic::host_upgrade_module_residuals::apply_choose_locomotor_set(
                u,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Panic,
                true,
            );
            u.ai_move_away_from_unit(rep_id, rep_pos);
            let _ = u.begin_request_safe_path(
                rep_id,
                u.move_away_destination.unwrap_or(rep_pos),
                self.frame,
            );
            true
        } else {
            false
        }
    }
}
