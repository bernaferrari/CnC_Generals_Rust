//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// C++ `AIPlayer::buildStructureWithDozer` (AIPlayer.cpp) — core path residual.
    ///
    /// findDozer → funds check → ground height → spawn + dozer build task →
    /// stamp BuildListInfo objectID/timestamp/underConstruction.
    /// C++ `AIPlayer::buildStructureWithDozer` (AIPlayer.cpp).
    ///
    /// findDozer → funds → ground Z → enemy-overlap reject → legalize/wiggle →
    /// path teleport residual → spawn UC building + dozer build task → stamp list.
    pub fn build_structure_with_dozer(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<Option<ObjectID>, AiError> {
        // Wave 255: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        // C++ findDozer may queueDozer internally; do not double-queue here.
        let Some(dozer_id) = self.find_dozer(&location)? else {
            return Ok(None);
        };

        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };

        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };

        let cost = template.calc_cost_to_build(Some(&*player_guard));
        if player_guard.get_money().get_money() < cost {
            return Ok(None);
        }

        let mut pos = location;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z += terrain.get_ground_height(pos.x, pos.y, None);
        }

        // C++ first check: BuildAssistant::NO_ENEMY_OBJECT_OVERLAP only.
        let enemy_only = FoundationValidator::from_build_options(
            LocalLegalToBuildOptions::NO_ENEMY_OBJECT_OVERLAP,
        );
        if enemy_only
            .validate_placement(&pos, template_name, angle, self.player_id as ObjectID)
            .is_err()
        {
            return Ok(None);
        }

        // C++ CLEAR_PATH | TERRAIN_RESTRICTIONS | NO_OBJECT_OVERLAP; wiggle if illegal.
        let validator = FoundationValidator::from_build_options(
            LocalLegalToBuildOptions::CLEAR_PATH
                | LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
        );
        let is_skirmish = self.is_skirmish_ai_player();
        let mut legal = validator
            .validate_placement(&pos, template_name, angle, self.player_id as ObjectID)
            .is_ok();
        if !legal {
            log::debug!(
                "{} - Dozer unable to place.  Attempting to adjust position.",
                template_name
            );
            let limit = if is_skirmish {
                120.0 * PATHFIND_CELL_SIZE_F
            } else {
                10.0 * PATHFIND_CELL_SIZE_F
            };
            let step = if is_skirmish {
                4.0 * PATHFIND_CELL_SIZE_F
            } else {
                2.0 * PATHFIND_CELL_SIZE_F
            };
            let mut pos_offset = 0.0_f32;
            let mut found = None;
            while pos_offset < limit {
                let offset = pos_offset * 0.5;
                // Horizontal edges at y = pos.y ± offset
                let mut x = pos.x - offset;
                let y0 = pos.y - offset;
                while x <= pos.x + offset + 0.001 {
                    for y in [y0, y0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, pos.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            found = Some(candidate);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    x += if is_skirmish {
                        2.0 * PATHFIND_CELL_SIZE_F
                    } else {
                        PATHFIND_CELL_SIZE_F
                    };
                }
                if found.is_some() {
                    break;
                }
                // Vertical edges at x = pos.x ± offset
                let mut y = pos.y - offset;
                let x0 = pos.x - offset;
                while y <= pos.y + offset + 0.001 {
                    for x in [x0, x0 + pos_offset] {
                        let candidate = Coord3D::new(x, y, pos.z);
                        if validator
                            .validate_placement(
                                &candidate,
                                template_name,
                                angle,
                                self.player_id as ObjectID,
                            )
                            .is_ok()
                        {
                            found = Some(candidate);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    y += if is_skirmish {
                        2.0 * PATHFIND_CELL_SIZE_F
                    } else {
                        PATHFIND_CELL_SIZE_F
                    };
                }
                if found.is_some() {
                    break;
                }
                pos_offset += step;
            }
            if let Some(p) = found {
                pos = p;
                legal = true;
            } else {
                // C++ final fallback: NO_ENEMY_OBJECT_OVERLAP only.
                legal = enemy_only
                    .validate_placement(&pos, template_name, angle, self.player_id as ObjectID)
                    .is_ok();
            }
        }
        if !legal {
            return Ok(None);
        }

        // C++: if (!pathfinder->clientSafeQuickDoesPathExist(
        //           dozer->getAI()->getLocomotorSet(), dozerPos, &pos))
        //        { log; dozer->setPosition(&pos); }
        let Some((dpos, loco_set)) = OBJECT_REGISTRY
            .with_object(dozer_id, |dozer_g| {
                let Some(dozer_ai) = dozer_g.get_ai_update_interface() else {
                    return None;
                };
                let dpos = *dozer_g.get_position();
                // Ensure Normal set is selected (C++ getLocomotorSet is current).
                dozer_ai.choose_locomotor_set(LocomotorSetType::Normal);
                let loco_set = dozer_ai.get_locomotor_set_clone();
                Some((dpos, loco_set))
            })
            .flatten()
        else {
            return Ok(None);
        };

        let mut path_ok = false;
        if let Some(ref loco_set) = loco_set {
            let ai_store = the_ai(); if let Ok(ai_guard) = ai_store.read() {
                if let Some(pf_arc) = ai_guard.pathfinder() {
                    if let Ok(pf) = pf_arc.read() {
                        path_ok = pf.client_safe_quick_does_path_exist(loco_set, &dpos, &pos);
                    }
                }
            }
        }
        // Empty/missing loco set → path_ok stays false → teleport (same as C++
        // when path fails; avoids always-teleport when loco data is present).
        if !path_ok {
            log::debug!(
                "{} - Dozer unable to reach building.  Teleporting.",
                template_name
            );
            let _ = OBJECT_REGISTRY.with_object_mut(dozer_id, |dozer_w| {
                let _ = dozer_w.set_position(&pos);
            });
        }

        let team = player_guard.get_default_team();
        drop(player_guard);
        drop(list);

        let Some(team_arc) = team else {
            return Ok(None);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(None);
        };
        let Ok(factory) = TheThingFactory::get() else {
            return Ok(None);
        };
        let mut starting_status = crate::common::ObjectStatusMaskType::NONE;
        if template.is_kind_of(crate::common::KindOf::Structure) {
            starting_status.set_status(crate::common::ObjectStatusTypes::UnderConstruction);
        }
        let Ok(new_object) =
            factory.new_object_with_status(template.clone(), &*team_guard, starting_status)
        else {
            return Ok(None);
        };
        drop(team_guard);

        let mut build_max_health = 0.0;
        if let Ok(guard) = new_object.read() {
            if let Some(body) = guard.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    build_max_health = body_guard.get_max_health();
                }
            }
        }

        let bldg_id = {
            let Ok(mut guard) = new_object.write() else {
                return Ok(None);
            };
            let _ = guard.set_position(&pos);
            let _ = guard.set_orientation(angle);
            let _ = OBJECT_REGISTRY.with_object(dozer_id, |dozer_g| {
                guard.set_producer(Some(dozer_g));
                guard.set_builder(Some(dozer_g));
            });
            guard.set_construction_percent(0.0);
            if build_max_health > 0.0 {
                let _ = guard.set_health(1.0);
            }
            guard.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction),
                true,
            );
            guard.get_id()
        };

        let total_build_frames = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .map(|pg| template.calc_time_to_build(Some(&*pg)).max(1) as u32)
            })
            .unwrap_or(300);

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(dozer_id, |dozer_g| dozer_g.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_g) = ai.try_lock() {
                if let Some(dozer_ai) = ai_g.get_dozer_ai_update_interface_mut() {
                    dozer_ai.set_build_task(bldg_id, total_build_frames, build_max_health, false);
                } else if let Some(worker_ai) = ai_g.get_worker_ai_update_interface_mut() {
                    worker_ai.set_build_task(bldg_id, total_build_frames, build_max_health, false);
                }
            }
        }

        // C++ stamps the BuildListInfo* passed into buildStructureWithDozer
        // (setObjectID/timestamp/underConstruction). Match that entry by template
        // + requested location so duplicate templates do not steal the stamp.
        // decrementNumRebuilds is done by caller in C++ processBaseBuilding; we
        // keep decrement here for solo process_base_building which does not.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        // Pass 1: prefer location match (C++ pointer identity).
                        let mut best_loc: Option<Coord3D> = None;
                        let mut best_dist = f32::MAX;
                        let mut fallback_loc: Option<Coord3D> = None;
                        {
                            let mut cur = Some(&*info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name
                                    && node.get_object_id() == INVALID_ID
                                {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - location.x;
                                    let dy = nloc.y - location.y;
                                    let d2 = dx * dx + dy * dy;
                                    if d2 < best_dist {
                                        best_dist = d2;
                                        best_loc = Some(nloc);
                                    }
                                    if fallback_loc.is_none() {
                                        fallback_loc = Some(nloc);
                                    }
                                }
                                cur = node.get_next();
                            }
                        }
                        // Exact-ish location first; else first free slot of that template.
                        let stamp_loc = if best_dist <= 1.0 {
                            best_loc
                        } else {
                            fallback_loc.or(best_loc)
                        };
                        if let Some(target) = stamp_loc {
                            let mut cur = Some(&mut *info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name
                                    && node.get_object_id() == INVALID_ID
                                {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - target.x;
                                    let dy = nloc.y - target.y;
                                    if dx * dx + dy * dy <= 1.0 {
                                        node.set_object_id(bldg_id);
                                        node.set_object_timestamp(
                                            TheGameLogic::get_frame().saturating_add(1),
                                        );
                                        node.set_under_construction(true);
                                        node.decrement_num_rebuilds();
                                        break;
                                    }
                                }
                                cur = node.get_next_mut();
                            }
                        }
                    }
                }
            }
        }

        log::debug!(
            "AI dozer {} started building {} as {}",
            dozer_id,
            template_name,
            bldg_id
        );
        Ok(Some(bldg_id))
    }

    /// C++ `AIPlayer::buildStructureNow` via priority residual (no BuildListInfo ptr).
    pub(super) fn build_structure_now(
        &mut self,
        priority: &ConstructionPriority,
    ) -> Result<(), AiError> {
        let location = if let Some(loc) = priority.desired_location {
            loc
        } else {
            self.calc_closest_construction_zone_near_base(&priority.building_type)?
                .unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
        };
        let angle = priority.desired_angle.unwrap_or(0.0);
        let _ = self.build_structure_now_at(&priority.building_type, location, angle, None)?;
        Ok(())
    }

    /// C++ `AIPlayer::buildStructureNow` (AIPlayer.cpp).
    ///
    /// Instant-construct (no dozer): BuildAssistant/new_object, clear UC status,
    /// stamp BuildListInfo, checkForSupplyCenter. Returns built object id.
    pub fn build_structure_now_at(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
        stamp_object_id_slot: Option<ObjectID>,
    ) -> Result<Option<ObjectID>, AiError> {
        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };

        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };
        let team = player_guard.get_default_team();
        drop(player_guard);
        drop(list);

        let Some(team_arc) = team else {
            return Ok(None);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(None);
        };
        let Ok(factory) = TheThingFactory::get() else {
            return Ok(None);
        };
        let mut starting_status = crate::common::ObjectStatusMaskType::NONE;
        if template.is_kind_of(crate::common::KindOf::Structure) {
            starting_status.set_status(crate::common::ObjectStatusTypes::UnderConstruction);
        }
        let Ok(new_object) =
            factory.new_object_with_status(template.clone(), &*team_guard, starting_status)
        else {
            return Ok(None);
        };
        drop(team_guard);

        let mut pos = location;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
        }

        // Capture BuildListInfo map props before/while stamping (C++ Dict).
        let mut map_building_name = String::new();
        let mut map_script = String::new();
        let mut map_health: i32 = 100;
        let mut map_unsellable = false;
        let mut stamped = false;

        // Prefer matching build-list entry props first (before object exists fully).
        // C++ uses the BuildListInfo* argument — match by slot id or template+location.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut best_dist = f32::MAX;
                    let mut fallback_done = false;
                    let mut cur = pg.get_build_list();
                    while let Some(node) = cur {
                        let name_match = node.get_template_name().as_str() == template_name;
                        if !name_match {
                            cur = node.get_next();
                            continue;
                        }
                        if let Some(id) = stamp_object_id_slot {
                            if node.get_object_id() == id {
                                map_building_name = node.get_building_name().to_string();
                                map_script = node.get_script().to_string();
                                map_health = node.get_health();
                                map_unsellable = node.get_unsellable();
                                best_dist = 0.0;
                                break;
                            }
                        }
                        if node.get_object_id() == INVALID_ID {
                            let nloc = *node.get_location();
                            let dx = nloc.x - location.x;
                            let dy = nloc.y - location.y;
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_dist {
                                best_dist = d2;
                                map_building_name = node.get_building_name().to_string();
                                map_script = node.get_script().to_string();
                                map_health = node.get_health();
                                map_unsellable = node.get_unsellable();
                            }
                            if !fallback_done {
                                // keep first free as last-resort if nothing closer later
                                fallback_done = true;
                                if best_dist == f32::MAX {
                                    map_building_name = node.get_building_name().to_string();
                                    map_script = node.get_script().to_string();
                                    map_health = node.get_health();
                                    map_unsellable = node.get_unsellable();
                                }
                            }
                        }
                        cur = node.get_next();
                    }
                }
            }
        }

        let bldg_id = {
            let Ok(mut guard) = new_object.write() else {
                return Ok(None);
            };
            let _ = guard.set_position(&pos);
            let _ = guard.set_orientation(angle);

            // C++ updateObjValuesFromMapProperties(Dict)
            let mut props = crate::common::Dict::new();
            props.set_ascii_string(
                crate::common::well_known_keys::key_object_name(),
                map_building_name.as_str(),
            );
            props.set_ascii_string(
                crate::common::well_known_keys::key_object_script_attachment(),
                map_script.as_str(),
            );
            props.set_int(
                crate::common::well_known_keys::key_object_initial_health(),
                map_health,
            );
            props.set_bool(
                crate::common::well_known_keys::key_object_unsellable(),
                map_unsellable,
            );
            guard.update_obj_values_from_map_properties(&props);

            // C++ clear UnderConstruction + Reconstructing (instant complete).
            let mask = ObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction)
                | ObjectStatusMaskType::from_status(ObjectStatusTypes::Reconstructing);
            guard.clear_status(mask);
            guard.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);
            // UnderConstruction just cleared → update upgrades (C++).
            guard.update_upgrade_modules_from_player();
            guard.get_id()
        };

        // Stamp build list entry: C++ stamps the BuildListInfo* passed in.
        // Prefer slot id hint, else template + requested location, else first free.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut best_loc: Option<Coord3D> = None;
                        let mut best_dist = f32::MAX;
                        let mut fallback_loc: Option<Coord3D> = None;
                        let mut slot_loc: Option<Coord3D> = None;
                        {
                            let mut cur = Some(&*info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() != template_name {
                                    cur = node.get_next();
                                    continue;
                                }
                                if let Some(id) = stamp_object_id_slot {
                                    if node.get_object_id() == id {
                                        slot_loc = Some(*node.get_location());
                                        break;
                                    }
                                }
                                if node.get_object_id() == INVALID_ID {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - location.x;
                                    let dy = nloc.y - location.y;
                                    let d2 = dx * dx + dy * dy;
                                    if d2 < best_dist {
                                        best_dist = d2;
                                        best_loc = Some(nloc);
                                    }
                                    if fallback_loc.is_none() {
                                        fallback_loc = Some(nloc);
                                    }
                                }
                                cur = node.get_next();
                            }
                        }
                        let stamp_loc = slot_loc.or(if best_dist <= 1.0 {
                            best_loc
                        } else {
                            fallback_loc.or(best_loc)
                        });
                        if let Some(target) = stamp_loc {
                            let mut cur = Some(&mut *info);
                            while let Some(node) = cur {
                                if node.get_template_name().as_str() == template_name {
                                    let nloc = *node.get_location();
                                    let dx = nloc.x - target.x;
                                    let dy = nloc.y - target.y;
                                    if dx * dx + dy * dy <= 1.0 {
                                        if stamp_object_id_slot
                                            .map(|id| node.get_object_id() == id)
                                            .unwrap_or(node.get_object_id() == INVALID_ID)
                                            || node.get_object_id() == INVALID_ID
                                        {
                                            node.set_object_id(bldg_id);
                                            node.set_object_timestamp(
                                                TheGameLogic::get_frame().saturating_add(1),
                                            );
                                            node.set_under_construction(false);
                                            stamped = true;
                                            break;
                                        }
                                    }
                                }
                                cur = node.get_next_mut();
                            }
                        }
                    }
                }
            }
        }
        let _ = stamped;

        // C++ TheScriptEngine->addObjectToCache + runObjectScript
        if let Ok(mut eng) = get_script_engine().write() {
            if let Some(e) = eng.as_mut() {
                e.add_object_to_cache(bldg_id);
                if !map_script.is_empty() {
                    e.run_object_script(&map_script, bldg_id);
                }
            }
        }

        // C++ checkForSupplyCenter(info, bldg)
        let _ = self.check_for_supply_center(bldg_id);

        // Rally offset residual deferred (gotOffset bug in C++ leaves gotOffset false).
        log::debug!("AI inst-built {} as {}", template_name, bldg_id);
        Ok(Some(bldg_id))
    }

    /// C++ `AIPlayer::startTraining` (AIPlayer.cpp).
    ///
    /// findFactory → ProductionUpdateInterface::queueCreateUnit(requestUniqueUnitID)
    /// → set order.factoryID. Returns true only if queued.
    pub(super) fn start_training_internal(
        &mut self,
        order: &mut WorkOrder,
        busy_ok: bool,
        team_name: &str,
    ) -> Result<bool, AiError> {
        // Wave 255: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let Some(factory_id) = self.find_factory_internal(&order.thing_template, busy_ok)? else {
            return Ok(false);
        };

        let Some(template) = TheThingFactory::find_template(&order.thing_template) else {
            return Ok(false);
        };

        // Prefer Object production queue path (queueCreateUnit + unique id).
        let Some(queued) = OBJECT_REGISTRY.with_object_mut(factory_id, |factory_g| {
            let production_id = factory_g.request_unique_unit_production_id().unwrap_or(0);
            if production_id != 0 {
                factory_g.queue_unit_with_production_id(&template, production_id)
            } else {
                factory_g.queue_unit(&template)
            }
        }) else {
            return Ok(false);
        };

        if !queued {
            // Fallback: ProductionUpdateInterface::start_production on behaviors.
            let Some(started) = OBJECT_REGISTRY.with_object(factory_id, |factory_g| {
                let mut started = false;
                for behavior in factory_g.get_behavior_modules() {
                    let Ok(mut bg) = behavior.lock() else {
                        continue;
                    };
                    let Some(prod) = bg.get_production_update_interface() else {
                        continue;
                    };
                    if prod
                        .start_production(order.thing_template.clone(), self.player_id)
                        .is_ok()
                    {
                        started = true;
                        break;
                    }
                }
                started
            }) else {
                return Ok(false);
            };
            if !started {
                return Ok(false);
            }
        }

        order.factory_id = Some(factory_id);
        log::debug!(
            "Queuing {} for {} at factory {}",
            order.thing_template,
            team_name,
            factory_id
        );
        Ok(true)
    }

    #[allow(dead_code)] // C++ parity: default wrapper for start_training_internal
    pub(super) fn start_training(&mut self, order: &mut WorkOrder) -> Result<(), AiError> {
        // Default: don't use busy factories
        self.start_training_internal(order, false, "default")?;
        Ok(())
    }

    /// Shared factory eligibility check used by build-list and object-scan paths.
    pub(super) fn factory_candidate(
        &self,
        obj_id: ObjectID,
        thing_template: &str,
        busy_ok: bool,
        busy_factory: &mut Option<ObjectID>,
    ) -> Result<Option<ObjectID>, AiError> {
        // Wave 255: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        let Some((module_handles, behaviors)) = OBJECT_REGISTRY
            .with_object(obj_id, |obj_guard| {
                if obj_guard.get_controlling_player_id() != Some(self.player_id) {
                    return None;
                }
                if obj_guard.is_destroyed()
                    || obj_guard.is_under_construction()
                    || obj_guard.test_status(ObjectStatusTypes::Sold)
                {
                    return None;
                }
                Some((
                    obj_guard.behavior_modules(),
                    obj_guard.get_behavior_modules(),
                ))
            })
            .flatten()
        else {
            return Ok(None);
        };

        let mut checked = false;
        for module_handle in module_handles {
            let mut can_produce = false;
            let mut is_busy = false;
            let matched = module_handle.with_module(|module| {
                let Some(prod) = module.get_production_control_interface() else {
                    return false;
                };
                if prod.can_produce(thing_template) {
                    can_produce = true;
                    is_busy = prod.is_producing() || prod.queue_size() > 0;
                }
                true
            });
            if matched {
                checked = true;
                if !can_produce {
                    return Ok(None);
                }
                if !is_busy {
                    return Ok(Some(obj_id));
                }
                if busy_ok && busy_factory.is_none() {
                    *busy_factory = Some(obj_id);
                }
                return Ok(None);
            }
        }

        if !checked {
            for behavior in behaviors {
                let Ok(mut behavior_guard) = behavior.lock() else {
                    continue;
                };
                let Some(prod) = behavior_guard.get_production_update_interface() else {
                    continue;
                };
                if !prod.can_produce(thing_template) {
                    continue;
                }
                let is_busy = prod.is_producing() || prod.get_queue_size() > 0;
                if !is_busy {
                    return Ok(Some(obj_id));
                }
                if busy_ok && busy_factory.is_none() {
                    *busy_factory = Some(obj_id);
                }
                break;
            }
        }

        Ok(None)
    }

    /// C++ `AIPlayer::findFactory` (AIPlayer.cpp).
    ///
    /// Iterates the player **build list only** (C++). Clears object IDs for
    /// captured factories. `busy_ok` allows returning a busy factory when no
    /// idle one exists (script priority teams).
    pub(super) fn find_factory_internal(
        &self,
        thing_template: &str,
        busy_ok: bool,
    ) -> Result<Option<ObjectID>, AiError> {
        // Wave 255: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        let mut busy_factory: Option<ObjectID> = None;
        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(None);
        };

        // --- C++ path: iterate build list only (no full-object scan). ---
        // Need mut build list to clear captured factory IDs like C++.
        drop(player_guard);
        drop(list);
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut player_guard) = player_arc.write() {
                    if let Some(head) = player_guard.get_build_list_mut() {
                        let mut current = Some(&mut *head);
                        while let Some(info) = current {
                            let obj_id = info.get_object_id();
                            if obj_id != INVALID_ID {
                                // C++: if factory->getControllingPlayer() != m_player → clear ID.
                                let wrong_owner = OBJECT_REGISTRY
                                    .with_object(obj_id, |g| {
                                        g.get_controlling_player_id() != Some(self.player_id)
                                    })
                                    .unwrap_or(false);
                                if wrong_owner {
                                    info.set_object_id(INVALID_ID);
                                } else if let Some(found) = self.factory_candidate(
                                    obj_id,
                                    thing_template,
                                    busy_ok,
                                    &mut busy_factory,
                                )? {
                                    return Ok(Some(found));
                                }
                            }
                            current = info.get_next_mut();
                        }
                    }
                }
            }
        }

        Ok(busy_factory)
    }

    pub(super) fn find_factory(&self, thing_template: &str) -> Result<Option<ObjectID>, AiError> {
        self.find_factory_internal(thing_template, false)
    }

    /// C++ `AIPlayer::selectTeamToBuild` (AIPlayer.cpp).
    ///
    /// 1. Collect isAGoodIdea candidates + hiPri
    /// 2. selectTeamToReinforce(hiPri) first
    /// 3. Random pick among hiPri set via GameLogicRandomValue
    /// 4. buildSpecificAITeam(low priority) + arm teamTimer with wealth mods
    pub(crate) fn select_team_to_build(&mut self) -> Result<bool, AiError> {
        const INVALID_PRI: i32 = -99999;

        // C++ iterates m_player->getPlayerTeams(), not the global TeamFactory.
        let candidates: Vec<(String, i32)> = {
            let Ok(list) = player_list().read() else {
                return Ok(false);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(false);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(false);
            };
            player_guard
                .get_player_team_prototypes()
                .iter()
                .map(|proto| {
                    (
                        proto.get_name().as_str().to_string(),
                        proto.get_production_priority(),
                    )
                })
                .collect()
        };

        let mut good: Vec<(String, i32)> = Vec::new();
        let mut hi_pri = INVALID_PRI;
        for (name, pri) in candidates {
            if self.is_a_good_idea_to_build_team(&name)? {
                if pri > hi_pri {
                    hi_pri = pri;
                }
                good.push((name, pri));
            }
        }

        // C++: try reinforce at hiPri before picking a new team.
        if self.select_team_to_reinforce(hi_pri)? {
            return Ok(true);
        }

        if hi_pri == INVALID_PRI {
            return Ok(false);
        }

        let hi: Vec<String> = good
            .into_iter()
            .filter(|(_, p)| *p == hi_pri)
            .map(|(n, _)| n)
            .collect();
        if hi.is_empty() {
            return Ok(false);
        }

        // C++ GameLogicRandomValue(0, count-1)
        let which = if hi.len() == 1 {
            0
        } else {
            game_logic_random_value(0, (hi.len() as u32) - 1) as usize
        };
        let team_name = &hi[which.min(hi.len() - 1)];

        // C++ buildSpecificAITeam(teamProto, false) — auto pick is low priority.
        // Low-priority path appends (push_back); frame_started already stamped inside.
        self.build_specific_ai_team(team_name, false)?;
        self.arm_team_timer_after_build()?;
        Ok(true)
    }

    /// After auto team select: C++ sets ready=false and teamTimer with wealth mods.
    ///
    /// Retail TeamSeconds=0 → timer 0 (like structureSeconds). C++ does not clamp
    /// to 1; next doTeamBuilding frame decrements and re-arms ready.
    pub(super) fn arm_team_timer_after_build(&mut self) -> Result<(), AiError> {
        self.ready_to_build_team = false;
        // C++: m_teamTimer = m_teamSeconds * LOGICFRAMES_PER_SECOND (0 is valid).
        let mut timer = (self.team_seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32) as u32;

        let money = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_money().get_money()))
            .unwrap_or(0);

        let (poor, wealthy, poor_mod, wealthy_mod) = Self::team_wealth_params();

        // C++: timer = timer / mod when mod applies (mod 0 → skip).
        // Integer divide of 0 stays 0 (immediate re-ready next doTeamBuilding).
        if money < poor && poor_mod > 0.0 {
            timer = (timer as f32 / poor_mod) as u32;
        } else if money > wealthy && wealthy_mod > 0.0 {
            timer = (timer as f32 / wealthy_mod) as u32;
        }
        self.team_timer = timer;
        Ok(())
    }

    /// C++ `AIPlayer::selectTeamToReinforce` (AIPlayer.cpp).
    ///
    /// Among auto-reinforce prototypes with priority > minPriority, find a live
    /// team instance missing units below maxUnits with an idle factory. Queue a
    /// single required work order (prepend), try recruit then startTraining,
    /// and shortcut teamDelay=0.
    pub(crate) fn select_team_to_reinforce(&mut self, min_priority: i32) -> Result<bool, AiError> {
        // C++ iterates m_player->getPlayerTeams() only.
        let protos: Vec<_> = {
            let Ok(list) = player_list().read() else {
                return Ok(false);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(false);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(false);
            };
            player_guard
                .get_player_team_prototypes()
                .iter()
                .cloned()
                .collect()
        };

        let mut best: Option<(String, Arc<RwLock<crate::team::Team>>, String, i32)> = None;
        // C++ curPriority starts at minPriority; only priorities *above* min win.
        let mut cur_priority = min_priority;

        for proto in &protos {
            if !proto.automatically_reinforce() {
                continue;
            }
            let priority = proto.get_production_priority();
            if priority <= cur_priority {
                continue;
            }
            let name = proto.get_name().as_str().to_string();

            // C++: busy if any TeamInQueue.m_team->getPrototype() == proto.
            let busy = self.team_build_queue.iter().any(|q| {
                if let Some(team_arc) = q.team.as_ref() {
                    if let Ok(tg) = team_arc.read() {
                        if tg.get_name().as_str() == name.as_str() {
                            return true;
                        }
                    }
                }
                q.team_name
                    .as_deref()
                    .map(|n| n == name.as_str())
                    .unwrap_or(false)
            });
            if busy {
                continue;
            }

            let Ok(factory_guard) = get_team_factory().lock() else {
                continue;
            };
            let instances = factory_guard.find_team_instances(&name);
            drop(factory_guard);

            for team_arc in instances {
                let Ok(team_g) = team_arc.read() else {
                    continue;
                };
                if !team_g.has_any_units() {
                    continue;
                }

                for unit_info in proto.units_info() {
                    if unit_info.max_units < 1 {
                        continue;
                    }
                    if unit_info.unit_thing_name.is_empty() {
                        continue;
                    }
                    let Some(thing) = TheThingFactory::find_template(unit_info.unit_thing_name)
                    else {
                        continue;
                    };
                    let mut counts = [0i32; 1];
                    team_g.count_objects_by_thing_template(
                        std::slice::from_ref(&thing),
                        false,
                        false,
                        &mut counts,
                    );
                    if counts[0] >= unit_info.max_units {
                        continue;
                    }
                    // Idle factory required (findFactory(thing, false)).
                    if self
                        .find_factory_internal(unit_info.unit_thing_name, false)?
                        .is_none()
                    {
                        continue;
                    }
                    // Better candidate.
                    best = Some((
                        name.clone(),
                        team_arc.clone(),
                        unit_info.unit_thing_name.to_string(),
                        priority,
                    ));
                    cur_priority = priority;
                }
            }
        }

        let Some((team_name, team_arc, thing_name, _)) = best else {
            return Ok(false);
        };

        let Some(thing) = TheThingFactory::find_template(&thing_name) else {
            return Ok(false);
        };

        // Origin: home location, else first member position.
        let (origin, _team_id) = {
            let Ok(team_g) = team_arc.read() else {
                return Ok(false);
            };
            let tid = team_g.get_id() as ObjectID;
            // C++: origin = homeLocation; if first member exists, use its position.
            let mut origin = Coord3D::new(0.0, 0.0, 0.0);
            if let Ok(factory) = get_team_factory().lock() {
                if let Some(proto) = factory.find_team_prototype(team_g.get_name().as_str()) {
                    if proto.has_home_location() {
                        origin = proto.home_location();
                    }
                }
            }
            if let Some(&mid) = team_g.get_members().first() {
                if let Some(pos) = OBJECT_REGISTRY.with_object(mid, |g| *g.get_position()) {
                    origin = pos;
                }
            }
            (origin, tid)
        };

        let ai_store = the_ai();let max_recruit = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.max_recruit_distance))
            .unwrap_or(99999.0);

        let mut order = WorkOrder::new(thing_name.clone());
        order.num_required = 1;
        order.required = true;
        order.factory_id = None;

        let mut recruited_id = None;
        if let Ok(team_g) = team_arc.read() {
            if let Some(unit_arc) = team_g.try_to_recruit(&thing, &origin, max_recruit) {
                // Transfer to this team + idle (C++ setTeam + aiIdle).
                if let Ok(mut unit_g) = unit_arc.write() {
                    let _ = unit_g.set_team(Some(team_arc.clone()));
                    if let Some(ai) = unit_g.get_ai_update_interface() {
                        ai.ai_idle(CommandSourceType::FromAi);
                    }
                    recruited_id = Some(unit_g.get_id());
                }
                order.num_completed = 1;
            }
        }

        if recruited_id.is_none() {
            // startTraining residual: assign factory if idle.
            let _ = self.start_training_internal(&mut order, false, &team_name)?;
        }

        let mut team_q = TeamInQueue::new();
        team_q.team_name = Some(team_name);
        team_q.team = Some(team_arc);
        team_q.priority_build = false;
        team_q.reinforcement = true;
        // C++ m_reinforcementID is the recruited unit, else INVALID until trained.
        team_q.reinforcement_id = recruited_id;
        team_q.frame_started = TheGameLogic::get_frame();
        team_q.work_orders.push(order);
        // C++ prependTo_TeamBuildQueue
        self.team_build_queue.push_front(team_q);
        // C++ m_teamDelay = 0 shortcut
        self.team_delay = 0;

        log::debug!("AI auto-reinforcing one {} onto team instance", thing_name);
        Ok(true)
    }
}
