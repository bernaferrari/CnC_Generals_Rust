//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// C++ `AIPlayer::queueSupplyTruck` (AIPlayer.cpp).
    ///
    /// Skip if a resource-gatherer is already queued. For each supply building
    /// needing gatherers: recount current, reattach loose harvesters, else start
    /// training one harvester (priority team) if under 3× desired global cap.
    /// C++ `AIPlayer::queueSupplyTruck` (AIPlayer.cpp).
    ///
    /// Skip if a resource-gatherer is already queued. For each supply build-list
    /// entry:
    /// - if current >= desired: maintain (nearby warehouse, recount/redock)
    /// - else: reattach loose harvesters, else train one (unless ≥3× desired total)
    pub(super) fn queue_supply_truck(&mut self) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Already building a supply truck?
        let truck_in_queue = self.team_build_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| order.is_resource_gatherer)
        });
        if truck_in_queue {
            return Ok(());
        }

        let total_harvesters = self.count_player_harvesters();

        // Snapshot supply-building build-list entries we may need to service.
        let mut supply_entries: Vec<(ObjectID, i32, i32)> = Vec::new();
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut cur = pg.get_build_list();
                    while let Some(info) = cur {
                        if info.is_supply_building() {
                            supply_entries.push((
                                info.get_object_id(),
                                info.get_desired_gatherers(),
                                info.get_current_gatherers(),
                            ));
                        }
                        cur = info.get_next();
                    }
                }
            }
        }

        for (center_id, desired, cur_gatherers) in supply_entries {
            if cur_gatherers >= desired {
                // C++ maintenance branch only when live non-hole center + nearby supplies.
                if center_id == INVALID_ID {
                    continue;
                }
                let Some(is_hole) = OBJECT_REGISTRY.with_object(center_id, |center_g| {
                    center_g.is_kind_of(KindOf::RebuildHole)
                }) else {
                    continue;
                };
                if is_hole {
                    continue;
                }
                // supply_center_has_nearby_supplies needs &Object - use with_object path via id helper if available
                let has_nearby = OBJECT_REGISTRY
                    .with_object(center_id, |center_g| {
                        self.supply_center_has_nearby_supplies(center_g)
                    })
                    .unwrap_or(false);
                if !has_nearby {
                    continue;
                }
                // C++ checkForSupplyCenter then recount docked harvesters.
                let _ = self.check_for_supply_center(center_id);
                let recounted = self.recount_and_redock_harvesters(center_id);
                self.set_build_list_current_gatherers(center_id, recounted);
                continue;
            }

            // Under-desired: reattach loose harvesters (preferred dock missing).
            if center_id != INVALID_ID {
                if self.try_reattach_loose_harvester(center_id)? {
                    return Ok(());
                }
            }

            if total_harvesters >= desired.saturating_mul(3) {
                continue; // lotsa gatherers
            }

            // Temporarily allow unit building while training a harvester.
            let prev_can_build = self.set_can_build_units_temp(true);
            let queued = self.queue_one_harvester_at_factory(center_id, cur_gatherers)?;
            self.set_can_build_units_temp(prev_can_build);
            if queued {
                return Ok(());
            }
        }

        Ok(())
    }

    pub(super) fn count_player_harvesters(&self) -> i32 {
        // Wave 255: empty dual-world → zero.

        if dual_world_registry_unavailable() {
            return 0;
        }

        let Ok(list) = player_list().read() else {
            return 0;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return 0;
        };
        let Ok(pg) = player_arc.read() else {
            return 0;
        };
        let mut total = 0;
        for obj_id in pg.get_all_objects() {
            let counts = OBJECT_REGISTRY
                .with_object(obj_id, |obj| {
                    if !obj.is_kind_of(KindOf::Harvester) {
                        return false;
                    }
                    let Some(ai) = obj.get_ai_update_interface() else {
                        return false;
                    };
                    ai.lock()
                        .ok()
                        .map(|ai_g| ai_g.get_supply_truck_ai_interface().is_some())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if counts {
                total += 1;
            }
        }
        total
    }

    pub(super) fn supply_center_has_nearby_supplies(&self, center: &Object) -> bool {
        // Wave 255: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }
        let center_pos = *center.get_position();
        let radius =
            SUPPLY_CENTER_CLOSE_DIST + center.get_geometry_info().get_bounding_circle_radius();

        let Some(partition) = ThePartitionManager::get() else {
            // Fallback: any warehouse on map with boxes.
            // Host path: empty dual-world registry → no warehouse residual.
            if OBJECT_REGISTRY.is_empty() {
                return false;
            }
            return OBJECT_REGISTRY.get_all_object_ids().iter().any(|obj_id| {
                let Some(obj) = OBJECT_REGISTRY.get_object(*obj_id) else {
                    return false;
                };
                obj.read()
                    .ok()
                    .map(|g| {
                        g.find_update_module("SupplyWarehouseDockUpdate")
                            .and_then(|m| {
                                m.with_module(|mm| {
                                    mm.get_supply_warehouse_dock_interface()
                                        .map(|w| w.boxes_stored() > 0)
                                })
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            });
        };

        for obj_id in partition.get_objects_in_range(&center_pos, radius) {
            let Some((their_team, team_arc, boxes)) = OBJECT_REGISTRY
                .with_object(obj_id, |obj| {
                    if !obj.is_kind_of(KindOf::SupplySource) {
                        return None;
                    }
                    let their_team = obj.get_controlling_player_id();
                    let team_arc = obj.get_team();
                    let boxes = obj
                        .find_update_module("SupplyWarehouseDockUpdate")
                        .and_then(|module| {
                            module.with_module(|m| {
                                m.get_supply_warehouse_dock_interface()
                                    .map(|w| w.boxes_stored())
                            })
                        })
                        .unwrap_or(0);
                    Some((their_team, team_arc, boxes))
                })
                .flatten()
            else {
                continue;
            };
            // Skip enemies.
            if let (Some(my_team), Some(their_team)) = (
                // approximate: controlling player
                Some(self.player_id),
                their_team,
            ) {
                if my_team != their_team {
                    // relationship residual: skip if not same player
                    // (C++ ENEMIES check via team relationship)
                    if let Ok(list) = player_list().read() {
                        if let Some(me) = list.get_player(self.player_id as i32) {
                            if let Ok(me_g) = me.read() {
                                if let Some(tarc) = team_arc {
                                    if let Ok(tg) = tarc.read() {
                                        if me_g.get_relationship_with_team(&tg)
                                            == Relationship::Enemies
                                        {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if boxes > 0 {
                return true;
            }
        }
        false
    }

    pub(super) fn recount_and_redock_harvesters(&self, center_id: ObjectID) -> i32 {
        // Wave 255: empty dual-world → zero.

        if dual_world_registry_unavailable() {
            return 0;
        }

        let Ok(list) = player_list().read() else {
            return 0;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return 0;
        };
        let Ok(pg) = player_arc.read() else {
            return 0;
        };
        let mut cur = 0;
        // Collect dock commands outside locks (C++ aiDock CMD_FROM_PLAYER).
        let mut redock: Vec<ObjectID> = Vec::new();
        for obj_id in pg.get_all_objects() {
            let Some((preferred, ferrying)) = OBJECT_REGISTRY
                .with_object(obj_id, |obj| {
                    if !obj.is_kind_of(KindOf::Harvester) {
                        return None;
                    }
                    let Some(ai) = obj.get_ai_update_interface() else {
                        return None;
                    };
                    let Ok(ai_g) = ai.lock() else {
                        return None;
                    };
                    let Some(truck) = ai_g.get_supply_truck_ai_interface() else {
                        return None;
                    };
                    Some((
                        truck.get_preferred_dock_id() == Some(center_id),
                        truck.is_currently_ferrying_supplies(),
                    ))
                })
                .flatten()
            else {
                continue;
            };
            if preferred {
                cur += 1;
                // C++: if (!isCurrentlyFerryingSupplies()) aiDock(center, CMD_FROM_PLAYER)
                if !ferrying {
                    redock.push(obj_id);
                }
            }
        }
        drop(pg);
        drop(list);
        for truck_id in redock {
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(truck_id, |obj| obj.get_ai_update_interface())
                .flatten()
            {
                ai.ai_dock(center_id, CommandSourceType::FromPlayer);
            }
        }
        cur
    }

    pub(super) fn set_build_list_current_gatherers(&self, center_id: ObjectID, cur: i32) {
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut node = Some(&mut *info);
                        while let Some(n) = node {
                            if n.get_object_id() == center_id {
                                n.set_current_gatherers(cur);
                                break;
                            }
                            node = n.get_next_mut();
                        }
                    }
                }
            }
        }
    }

    pub(super) fn try_reattach_loose_harvester(
        &mut self,
        center_id: ObjectID,
    ) -> Result<bool, AiError> {
        // Wave 255: empty dual-world → Ok(false).

        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(false);
        };
        let Ok(pg) = player_arc.read() else {
            return Ok(false);
        };
        for obj_id in pg.get_all_objects() {
            let Some(should_reattach) = OBJECT_REGISTRY
                .with_object(obj_id, |obj| {
                    if !obj.is_kind_of(KindOf::Harvester) {
                        return None;
                    }
                    let Some(ai) = obj.get_ai_update_interface() else {
                        return None;
                    };
                    let Ok(ai_g) = ai.lock() else {
                        return None;
                    };
                    let Some(truck) = ai_g.get_supply_truck_ai_interface() else {
                        return None;
                    };
                    let dock = truck.get_preferred_dock_id();
                    let dock_alive = dock
                        .map(|id| OBJECT_REGISTRY.with_object(id, |_| ()).is_some())
                        .unwrap_or(false);
                    if dock_alive {
                        return Some(false);
                    }
                    Some(
                        truck.is_currently_ferrying_supplies()
                            || truck.is_forced_into_wanting_state(),
                    )
                })
                .flatten()
            else {
                continue;
            };
            if should_reattach {
                // C++: bump current gatherers and aiDock(center, CMD_FROM_PLAYER).
                // Issue dock before recount so preferred dock can stick.
                if let Some(ai) = OBJECT_REGISTRY
                    .with_object(obj_id, |og| og.get_ai_update_interface())
                    .flatten()
                {
                    ai.ai_dock(center_id, CommandSourceType::FromPlayer);
                }
                self.set_build_list_current_gatherers(
                    center_id,
                    self.recount_and_redock_harvesters(center_id),
                );
                log::debug!("Re-attaching supply truck to supply center.");
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn set_can_build_units_temp(&self, can: bool) -> bool {
        let Ok(list) = player_list().read() else {
            return can;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return can;
        };
        let Ok(mut pg) = player_arc.write() else {
            return can;
        };
        let prev = pg.get_can_build_units();
        pg.set_can_build_units(can);
        prev
    }

    /// Find a harvester template with an idle factory and queue one (C++ priority team).
    ///
    /// C++ walks `TheThingFactory->firstTemplate()` / `friend_getNextTemplate()` for
    /// `KINDOF_HARVESTER`. Fall back to known faction names if the factory is empty.
    pub(super) fn queue_one_harvester_at_factory(
        &mut self,
        center_id: ObjectID,
        cur_gatherers: i32,
    ) -> Result<bool, AiError> {
        // Collect harvester template names: full factory walk first (C++ order).
        let mut harvester_names: Vec<String> = Vec::new();
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut current = factory.first_template().cloned();
                while let Some(template) = current {
                    // Common ThingTemplate uses u64 masks; resolve via TheThingFactory
                    // adapter for KindOf::Harvester (C++ isKindOf(KINDOF_HARVESTER)).
                    let name = template.get_name().to_string();
                    if !name.is_empty()
                        && TheThingFactory::find_template(&name)
                            .map(|t| t.is_kind_of(KindOf::Harvester))
                            .unwrap_or(false)
                        && !harvester_names.iter().any(|n| n == &name)
                    {
                        harvester_names.push(name);
                    }
                    current = template.get_next_template().clone();
                }
            }
        }
        // Fallback residual when ThingFactory unloaded (tests / early boot).
        if harvester_names.is_empty() {
            for name in [
                "AmericaVehicleChinook",
                "AmericaVehicleSupplyTruck",
                "ChinaVehicleSupplyTruck",
                "GLAVehicleSupplyTruck",
                "GLAInfantryWorker",
                "SupplyTruck",
            ] {
                if TheThingFactory::find_template(name)
                    .map(|t| t.is_kind_of(KindOf::Harvester))
                    .unwrap_or(false)
                {
                    harvester_names.push(name.to_string());
                }
            }
        }

        for name in harvester_names {
            let Some(factory_id) = self.find_factory_internal(&name, false)? else {
                continue;
            };

            let mut order = WorkOrder::new(name.clone());
            order.num_required = 1;
            order.required = true;
            order.is_resource_gatherer = true;

            let mut team = TeamInQueue::new();
            team.priority_build = true;
            team.frame_started = TheGameLogic::get_frame();
            // C++ sticks supply truck on default team (m_team + name).
            if let Ok(list) = player_list().read() {
                if let Some(player_arc) = list.get_player(self.player_id as i32) {
                    if let Ok(pg) = player_arc.read() {
                        if let Some(dt) = pg.get_default_team() {
                            if let Ok(tg) = dt.read() {
                                team.team_name = Some(tg.get_name().to_string());
                            }
                            team.team = Some(dt);
                        }
                    }
                }
            }

            self.team_delay = 0;
            let team_name = team
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            if cur_gatherers == -1 {
                // First one is automatic (C++): assign factory without training.
                order.factory_id = Some(factory_id);
                self.set_build_list_current_gatherers(center_id, 0);
                team.work_orders.push(order);
                self.team_build_queue.push_front(team);
                log::debug!(
                    "Supply truck - automatic first gatherer ({}) at factory {}",
                    name,
                    factory_id
                );
                return Ok(true);
            }

            // startTraining before push to avoid double borrow of self.
            let _ = self.start_training_internal(&mut order, true, &team_name)?;
            team.work_orders.push(order);
            self.team_build_queue.push_front(team);
            log::debug!(
                "Supply truck - building one {} at factory {}",
                name,
                factory_id
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// C++ `AIPlayer::processBaseBuilding` (AIPlayer.cpp) — USE_DOZER path residual.
    ///
    /// Walk player build list: track destroyed buildings, honor rebuild delay,
    /// start at most one dozer build per call, then arm structureTimer with wealth mods.
    pub(super) fn process_base_building(&mut self) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !self.ready_to_build_structure {
            return Ok(());
        }

        // C++ processBaseBuilding: build list walk only (no host priority analysis).
        let current_frame = TheGameLogic::get_frame();
        let rebuild_delay_frames = self.rebuild_delay_frames();

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return Ok(());
        };
        let player_index = player_guard.get_player_index() as u32;

        // Collect first actionable missing buildable entry (name, location, angle).
        // Also collect under-construction buildings needing dozer resume (C++).
        let mut to_build: Option<(String, Coord3D, Real)> = None;
        let mut resume_jobs: Vec<(ObjectID, ObjectID, Coord3D)> = Vec::new();
        // (bldg_id, builder_id_or_INVALID, bldg_pos)
        let mut info_opt = player_guard.get_build_list_mut();
        while let Some(info) = info_opt {
            let name = info.get_template_name();
            if name.is_empty() {
                info_opt = info.get_next_mut();
                continue;
            }

            let obj_id = info.get_object_id();
            if obj_id != INVALID_ID {
                // Some((owned, job)): object exists. None: missing.
                match OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                    if obj_guard.get_controlling_player_id() == Some(player_index) {
                        let under = obj_guard.test_status(ObjectStatusTypes::UnderConstruction)
                            || obj_guard.is_under_construction();
                        let job = if under {
                            Some((
                                obj_id,
                                obj_guard.get_builder_id(),
                                *obj_guard.get_position(),
                            ))
                        } else {
                            None
                        };
                        (true, job)
                    } else {
                        (false, None)
                    }
                }) {
                    Some((true, job)) => {
                        if let Some(job) = job {
                            resume_jobs.push(job);
                        }
                        info_opt = info.get_next_mut();
                        continue;
                    }
                    Some((false, _)) | None => {
                        // Captured or gone: clear and stamp for rebuild delay.
                        let prior_id = obj_id;
                        info.set_object_id(INVALID_ID);
                        info.set_object_timestamp(current_frame.saturating_add(1));
                        // C++ GLA hole scan by spawnerID.
                        // Host path: empty dual-world registry → no rebuild-hole residual.
                        if !OBJECT_REGISTRY.is_empty() {
                            for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
                                let hole_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                                    Some(v) => v,
                                    None => continue,
                                };
                                let Ok(hg) = hole_arc.read() else {
                                    continue;
                                };
                                if !hg.is_kind_of(KindOf::RebuildHole) {
                                    continue;
                                }
                                let mut matched = false;
                                for behavior in hg.get_behavior_modules() {
                                    if let Ok(mut bg) = behavior.lock() {
                                        if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface()
                                        {
                                            if rhbi.get_spawner_id() == prior_id {
                                                matched = true;
                                            }
                                            break;
                                        }
                                    }
                                }
                                if matched {
                                    info.set_object_id(hg.get_id());
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // C++: only apply rebuild delay when objectID is INVALID and timestamp>0.
            // (Hole-attached IDs skip this branch until the hole is gone.)
            if info.get_object_id() == INVALID_ID && info.get_object_timestamp() > 0 {
                if info
                    .get_object_timestamp()
                    .saturating_add(rebuild_delay_frames)
                    > current_frame
                {
                    info_opt = info.get_next_mut();
                    continue;
                }
                log::debug!("Enabling rebuild for {}", name);
                info.set_object_timestamp(0); // ready to build
            }

            if !info.is_buildable() {
                info_opt = info.get_next_mut();
                continue;
            }

            // C++: isBuildable && findObjectByID == NULL → dozer build.
            if info.get_object_id() == INVALID_ID {
                to_build = Some((name.to_string(), *info.get_location(), info.get_angle()));
                break;
            }

            info_opt = info.get_next_mut();
        }
        drop(player_guard);

        // C++: for each UC building, aiResumeConstruction on builder or findDozer.
        for (bldg_id, builder_id, bldg_pos) in resume_jobs {
            let mut dozer_id = builder_id;
            let mut builder_ok = false;
            if dozer_id != INVALID_ID {
                if OBJECT_REGISTRY
                    .with_object(dozer_id, |dg| {
                        dg.get_controlling_player_id() == Some(player_index)
                            && dg.get_ai_update_interface().is_some()
                    })
                    .unwrap_or(false)
                {
                    builder_ok = true;
                }
            }
            if !builder_ok {
                log::debug!("AI's Dozer got killed.  Find another dozer.");
                // C++ solo does not queueDozer here (skirmish does).
                dozer_id = self.find_dozer(&bldg_pos)?.unwrap_or(INVALID_ID);
                if dozer_id == INVALID_ID {
                    continue;
                }
                // Clear dead builder on building.
                let _ = OBJECT_REGISTRY.with_object_mut(bldg_id, |bg| {
                    bg.set_builder(None);
                });
            }
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(dozer_id, |dg| dg.get_ai_update_interface())
                .flatten()
            {
                if let Ok(mut ai_g) = ai.lock() {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::ResumeConstruction,
                        CommandSourceType::FromAi,
                    );
                    params.obj = Some(bldg_id);
                    let _ = ai_g.execute_command(&params);
                }
            }
        }

        if let Some((name, location, angle)) = to_build {
            // C++ USE_DOZER: buildStructureWithDozer; NULL → no timer arm.
            match self.build_structure_with_dozer(&name, location, angle)? {
                Some(_bldg_id) => {
                    self.arm_structure_timer_after_build()?;
                    self.frame_last_building_built = current_frame;
                    // C++: only one building per delay loop.
                    return Ok(());
                }
                None => {
                    // No dozer / funds / placement — retry later.
                    return Ok(());
                }
            }
        }

        // C++ processBaseBuilding walks BuildListInfo only — no construction_priorities fallback.
        Ok(())
    }

    /// C++ rebuild delay frames from AIData `m_rebuildDelaySeconds` (default path).
    /// Retail AIData = 30; zero/unloaded AIData falls back to REBUILD_DELAY_SECONDS.
    pub(super) fn rebuild_delay_frames(&self) -> u32 {
        let ai_store = the_ai();let seconds = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    if data.rebuild_delay_seconds > 0 {
                        data.rebuild_delay_seconds as u32
                    } else {
                        REBUILD_DELAY_SECONDS
                    }
                })
            })
            .unwrap_or(REBUILD_DELAY_SECONDS);
        seconds * LOGICFRAMES_PER_SECOND
    }

    /// After starting a structure: C++ sets ready=false and structureTimer with wealth mods.
    ///
    /// C++ always re-reads `TheAI->getAiData()->m_structureSeconds` (not a player
    /// field). Retail StructureSeconds=0 → timer 0 (immediately eligible next
    /// doBaseBuilding).
    pub(crate) fn arm_structure_timer_after_build(&mut self) -> Result<(), AiError> {
        self.ready_to_build_structure = false;
        // Live AIData structureSeconds (0.0 is valid retail). Keep field snapshot
        // in sync for xfer/tests that set structure_seconds directly.
        let ai_store = the_ai();let structure_seconds = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.structure_seconds))
            .unwrap_or(self.structure_seconds);
        self.structure_seconds = structure_seconds;
        let mut timer = (structure_seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32) as u32;

        let money = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()))
            .unwrap_or(0);

        let (poor, wealthy, poor_mod, wealthy_mod) = Self::structure_wealth_params();

        // C++: timer = timer / mod when mod applies (mod 0 → skip).
        // Integer divide of 0 stays 0 (immediate re-ready).
        if money < poor && poor_mod > 0.0 {
            timer = (timer as f32 / poor_mod) as u32;
        } else if money > wealthy && wealthy_mod > 0.0 {
            timer = (timer as f32 / wealthy_mod) as u32;
        }

        self.structure_timer = timer;
        Ok(())
    }

    /// Retail AIData structure wealth params; zero AIData fields → Default/AIData.ini fallbacks.
    pub(super) fn structure_wealth_params() -> (i32, i32, f32, f32) {
        the_ai()
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            RESOURCES_WEALTHY
                        },
                        if data.structures_poor_mod > 0.0 {
                            data.structures_poor_mod
                        } else {
                            STRUCTURES_POOR_MODIFIER
                        },
                        if data.structures_wealthy_mod > 0.0 {
                            data.structures_wealthy_mod
                        } else {
                            STRUCTURES_WEALTHY_MODIFIER
                        },
                    )
                })
            })
            .unwrap_or((
                RESOURCES_POOR,
                RESOURCES_WEALTHY,
                STRUCTURES_POOR_MODIFIER,
                STRUCTURES_WEALTHY_MODIFIER,
            ))
    }

    /// Retail AIData team wealth params; zero AIData fields → Default/AIData.ini fallbacks.
    pub(super) fn team_wealth_params() -> (i32, i32, f32, f32) {
        the_ai()
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data().read().ok().map(|data| {
                    (
                        if data.resources_poor > 0 {
                            data.resources_poor
                        } else {
                            RESOURCES_POOR
                        },
                        if data.resources_wealthy > 0 {
                            data.resources_wealthy
                        } else {
                            RESOURCES_WEALTHY
                        },
                        if data.team_poor_mod > 0.0 {
                            data.team_poor_mod
                        } else {
                            TEAMS_POOR_MODIFIER
                        },
                        if data.team_wealthy_mod > 0.0 {
                            data.team_wealthy_mod
                        } else {
                            TEAMS_WEALTHY_MODIFIER
                        },
                    )
                })
            })
            .unwrap_or((
                RESOURCES_POOR,
                RESOURCES_WEALTHY,
                TEAMS_POOR_MODIFIER,
                TEAMS_WEALTHY_MODIFIER,
            ))
    }

    /// Analyze current building needs
    pub(super) fn analyze_building_needs(&mut self) -> Result<(), AiError> {
        // Check if we need power
        if self.economic_state.power_shortage {
            let priority = ConstructionPriority {
                building_type: "PowerPlant".to_string(),
                priority: 1,
                prerequisites_met: true,
                max_count: None,
                current_count: 0,
                desired_location: None,
                desired_angle: None,
            };
            self.construction_priorities.push(priority);
        }

        // Check if we need supply centers
        if self.economic_state.supply_shortage {
            let priority = ConstructionPriority {
                building_type: "SupplyCenter".to_string(),
                priority: 2,
                prerequisites_met: true,
                max_count: None,
                current_count: 0,
                desired_location: None,
                desired_angle: None,
            };
            self.construction_priorities.push(priority);
        }

        Ok(())
    }
}
