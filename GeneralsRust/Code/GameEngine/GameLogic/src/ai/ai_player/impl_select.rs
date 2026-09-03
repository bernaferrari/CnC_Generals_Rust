//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// C++ `AIPlayer::isPossibleToBuildTeam` (AIPlayer.cpp).
    ///
    /// Returns `(possible, not_enough_money)`.
    /// For each unit type: must have *some* factory (`findFactory(..., true)`);
    /// track whether any unit type has an **idle** factory. Cost uses
    /// `(min+max)/2` average count, then `* teamResourcesToBuild`.
    pub(super) fn is_possible_to_build_team(
        &self,
        team_name: &str,
        require_idle_factory: bool,
    ) -> Result<(bool, bool), AiError> {
        let factory = get_team_factory();
        let Ok(factory_guard) = factory.lock() else {
            return Ok((false, false));
        };
        let Some(proto) = factory_guard.find_team_prototype(team_name) else {
            return Ok((false, false));
        };
        // Clone unit list so we can drop the factory lock before find_factory.
        let units: Vec<(String, i32, i32)> = proto
            .units_info()
            .iter()
            .filter(|u| !u.unit_thing_name.is_empty())
            .map(|u| (u.unit_thing_name.to_string(), u.min_units, u.max_units))
            .collect();
        drop(factory_guard);

        // Cost calc needs player, but find_factory_internal also locks the player
        // RwLock (not reentrant) — snapshot money/cost player handle briefly, drop,
        // then factory-scan, then re-check money.
        // C++ uses Int cost with float intermediate truncated each assignment.
        let mut any_idle = false;
        let mut cost: i32 = 0;
        {
            let Ok(list) = player_list().read() else {
                return Ok((false, false));
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok((false, false));
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok((false, false));
            };
            for (thing_name, min_units, max_units) in &units {
                let Some(template) = TheThingFactory::find_template(thing_name) else {
                    continue;
                };
                let thing_cost = template.calc_cost_to_build(Some(&*player_guard)) as i32;
                // C++: cost += thingCost * ((maxUnits+minUnits)/2.0f);  // truncates to Int
                cost +=
                    (thing_cost as f32 * ((*max_units as f32 + *min_units as f32) / 2.0)) as i32;
            }
        }

        for (thing_name, _min_units, _max_units) in &units {
            if TheThingFactory::find_template(thing_name).is_none() {
                continue;
            }
            // C++: findFactory(thing, true) — any factory (busy OK). Missing → false.
            if self.find_factory_internal(thing_name, true)?.is_none() {
                return Ok((false, false));
            }
            // C++: findFactory(thing, false) — idle.
            if self.find_factory_internal(thing_name, false)?.is_some() {
                any_idle = true;
            }
        }

        let ai_store = the_ai();let resources_mod = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.team_resources_to_build)
            })
            .filter(|m| *m > 0.0)
            .unwrap_or(TEAM_RESOURCES_TO_BUILD);
        // C++: cost *= m_teamResourcesToBuild; (Int *= Real truncates)
        cost = (cost as f32 * resources_mod) as i32;

        let money = {
            let Ok(list) = player_list().read() else {
                return Ok((false, false));
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok((false, false));
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok((false, false));
            };
            player_guard.get_money().get_money() as i32
        };
        if money < cost {
            return Ok((false, true)); // notEnoughMoney
        }
        if any_idle {
            return Ok((true, false));
        }
        if !require_idle_factory {
            return Ok((true, false));
        }
        Ok((false, false))
    }

    /// Check if team is a good idea to build right now
    /// Matches C++ AIPlayer.cpp:1471 isAGoodIdeaToBuildTeam
    pub(crate) fn is_a_good_idea_to_build_team(&self, team_name: &str) -> Result<bool, AiError> {
        // C++ AIPlayer::isAGoodIdeaToBuildTeam:
        // 1. evaluateProductionCondition()
        // 2. countTeamInstances() >= maxInstances → reject
        // 3. already building same prototype in TeamBuildQueue → reject
        // 4. isPossibleToBuildTeam(proto, true, needMoney)

        // Snapshot under the factory lock, then drop before is_possible_to_build_team
        // (same Mutex — not reentrant).
        let (condition_ok, instances, max_instances) = {
            let factory = get_team_factory();
            let Ok(factory_guard) = factory.lock() else {
                return Ok(false);
            };
            let Some(proto) = factory_guard.find_team_prototype(team_name) else {
                return Ok(false);
            };
            (
                proto.evaluate_production_condition(),
                factory_guard.find_team_instances(team_name).len() as i32,
                proto.get_max_instances(),
            )
        };

        if !condition_ok {
            return Ok(false);
        }
        // C++ bare: countTeamInstances() >= m_maxInstances
        if instances >= max_instances {
            return Ok(false);
        }

        // C++: team->m_team->getPrototype() == proto (busy building this prototype).
        if self.team_build_queue.iter().any(|q| {
            if let Some(team_arc) = q.team.as_ref() {
                if let Ok(tg) = team_arc.read() {
                    if tg.get_name().as_str() == team_name {
                        return true;
                    }
                }
            }
            q.team_name
                .as_deref()
                .map(|name| name == team_name)
                .unwrap_or(false)
        }) {
            return Ok(false);
        }

        let (possible, _) = self.is_possible_to_build_team(team_name, true)?;
        Ok(possible)
    }

    /// C++ `AIPlayer::findDozer` (AIPlayer.cpp).
    ///
    /// Prefer idle dozers (not building, not ferrying supplies, not repair dozer).
    /// Closest idle dozer wins. If no dozer exists at all, queue one.
    pub(super) fn find_dozer(&mut self, location: &Coord3D) -> Result<Option<ObjectID>, AiError> {
        // Wave 255: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        use crate::object::update::ai_update::dozer_ai_update::DozerTask;

        let mut need_dozer = true;
        let mut dozer: Option<ObjectID> = None;
        let mut closest_dozer: Option<ObjectID> = None;
        let mut closest_dist_sqr = 0.0_f32;

        let object_ids: Vec<ObjectID> = {
            let Ok(list) = player_list().read() else {
                return Ok(None);
            };
            let Some(player_arc) = list.get_player(self.player_id as i32) else {
                return Ok(None);
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(None);
            };
            player_guard.get_all_objects()
        };

        for obj_id in object_ids {
            let Some((build_pending, any_pending, ferrying, pos)) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if obj_guard.is_destroyed() || !obj_guard.is_kind_of(KindOf::Dozer) {
                        return None;
                    }
                    let Some(ai) = obj_guard.get_ai_update_interface() else {
                        return None;
                    };
                    let Ok(mut ai_guard) = ai.lock() else {
                        return None;
                    };

                    // Must have dozer AI; capture task flags before optional truck check.
                    let (has_dozer, build_pending, any_pending) =
                        match ai_guard.get_dozer_ai_update_interface_mut() {
                            Some(dozer_ai) => (
                                true,
                                dozer_ai.is_task_pending(DozerTask::Build),
                                dozer_ai.is_any_task_pending(),
                            ),
                            None => (false, false, false),
                        };
                    if !has_dozer {
                        return None;
                    }
                    let ferrying = if !any_pending {
                        // Don't steal supply-ferrying workers (GLA).
                        ai_guard
                            .get_supply_truck_ai_interface()
                            .map(|truck| {
                                truck.is_currently_ferrying_supplies()
                                    || truck.is_forced_into_wanting_state()
                            })
                            .unwrap_or(false)
                    } else {
                        false
                    };
                    Some((
                        build_pending,
                        any_pending,
                        ferrying,
                        *obj_guard.get_position(),
                    ))
                })
                .flatten()
            else {
                continue;
            };
            if ferrying {
                continue;
            }

            if Some(obj_id) == self.repair_dozer {
                continue;
            }
            need_dozer = false;

            if build_pending {
                continue;
            }
            let idle = !any_pending;
            if idle {
                dozer = Some(obj_id);
            } else if dozer.is_none() {
                dozer = Some(obj_id);
            }

            if idle {
                let dx = location.x - pos.x;
                let dy = location.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if closest_dozer.is_none() || dist_sqr < closest_dist_sqr {
                    closest_dozer = Some(obj_id);
                    closest_dist_sqr = dist_sqr;
                }
            }
        }

        if need_dozer {
            let _ = self.queue_dozer();
        }
        if closest_dozer.is_some() {
            return Ok(closest_dozer);
        }
        Ok(dozer)
    }

    /// C++ `AIPlayer::queueDozer` (AIPlayer.cpp).
    ///
    /// If no dozer already queued, walk ThingFactory for KINDOF_DOZER with a
    /// factory (busyOK=true), priority-queue a team, and startTraining.
    /// Does **not** set `dozer_queued_for_repair` (that flag is repair-path only).
    pub(crate) fn queue_dozer(&mut self) -> Result<(), AiError> {
        if self.dozer_in_queue() {
            return Ok(());
        }

        let prev_can = self.set_can_build_units_temp(true);

        // C++: firstTemplate / friend_getNextTemplate for KINDOF_DOZER.
        let mut dozer_names: Vec<String> = Vec::new();
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut current = factory.first_template().cloned();
                while let Some(template) = current {
                    let name = template.get_name().to_string();
                    if !name.is_empty()
                        && TheThingFactory::find_template(&name)
                            .map(|t| t.is_kind_of(KindOf::Dozer))
                            .unwrap_or(false)
                        && !dozer_names.iter().any(|n| n == &name)
                    {
                        dozer_names.push(name);
                    }
                    current = template.get_next_template().clone();
                }
            }
        }
        // Fallback residual when ThingFactory unloaded (tests / early boot).
        if dozer_names.is_empty() {
            for name in [
                "AmericaVehicleDozer",
                "ChinaVehicleDozer",
                "GLAInfantryWorker",
                "Dozer",
                "Worker",
            ] {
                if TheThingFactory::find_template(name)
                    .map(|t| t.is_kind_of(KindOf::Dozer))
                    .unwrap_or(false)
                {
                    dozer_names.push(name.to_string());
                }
            }
        }

        for name in dozer_names {
            // C++ findFactory(tTemplate, true) — busyOK allows queueing on busy factory.
            let Some(factory_id) = self.find_factory_internal(&name, true)? else {
                continue;
            };

            let mut order = WorkOrder::new(name.clone());
            order.num_required = 1;
            order.required = true;
            order.is_resource_gatherer = false;

            let mut team = TeamInQueue::new();
            team.priority_build = true;
            team.frame_started = TheGameLogic::get_frame();
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
            let team_name = team
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            // C++: prependTo_TeamBuildQueue then startTraining. Train first so we
            // do not hold a queue borrow across &mut self (same observable result:
            // factoryID stamped on the order before it sits in the queue).
            self.team_delay = 0;
            let _ = self.start_training_internal(&mut order, true, &team_name)?;
            team.work_orders.push(order);
            self.team_build_queue.push_front(team);
            // C++ queueDozer does not set m_dozerQueuedForRepair.
            log::debug!("DOZER - building one {} at factory {}", name, factory_id);
            break;
        }

        self.set_can_build_units_temp(prev_can);
        Ok(())
    }

    /// Returns true if a dozer is already present in the build queue.
    /// C++ `dozerInQueue` → `TeamInQueue::includesADozer` (KINDOF_DOZER and
    /// **not** a resource-gatherer work order — GLA workers can be both).
    pub fn dozer_in_queue(&self) -> bool {
        self.team_build_queue
            .iter()
            .any(|team| team.includes_a_dozer())
    }

    /// C++ `AIPlayer::repairStructure` (AIPlayer.cpp).
    pub(crate) fn repair_structure(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        let Some(body) = OBJECT_REGISTRY
            .with_object(structure_id, |structure_g| structure_g.get_body_module())
            .flatten()
        else {
            return Ok(());
        };
        let Ok(body_g) = body.lock() else {
            return Ok(());
        };
        if body_g.get_damage_state() == crate::object::body::BodyDamageType::Pristine {
            return Ok(());
        }
        drop(body_g);

        // Already queued?
        for i in 0..self.structures_in_queue as usize {
            if self.structures_to_repair.get(i).and_then(|s| *s) == Some(structure_id) {
                return Ok(());
            }
        }
        if self.structures_in_queue as usize >= MAX_STRUCTURES_TO_REPAIR {
            log::debug!("Structure repair queue is full, ignoring repair request.");
            return Ok(());
        }
        let idx = self.structures_in_queue as usize;
        self.structures_to_repair[idx] = Some(structure_id);
        self.structures_in_queue += 1;
        Ok(())
    }

    /// Remove all queued teams from both the build and ready queues.
    pub fn clear_teams_in_queue(&mut self) {
        self.team_build_queue.clear();
        self.team_ready_queue.clear();
    }

    pub fn set_base_center_set(&mut self, set: bool) {
        self.base_center_set = set;
        if !set {
            self.base_radius = 0.0;
        }
    }

    /// Public wrapper for skirmish newMap initiallyBuilt inst-build.
    pub fn build_structure_now_at_public(
        &mut self,
        template_name: &str,
        location: Coord3D,
        angle: Real,
    ) -> Result<Option<ObjectID>, AiError> {
        self.build_structure_now_at(template_name, location, angle, None)
    }

    /// Public findDozer for skirmish processBaseBuilding resume path.
    pub fn find_dozer_public(&mut self, location: &Coord3D) -> Result<Option<ObjectID>, AiError> {
        self.find_dozer(location)
    }

    pub fn set_frame_last_building_built(&mut self, frame: u32) {
        self.frame_last_building_built = frame;
    }

    /// C++ `AIPlayer::aiPreTeamDestroy(const Team *deletedTeam)`.
    ///
    /// Drop TeamInQueue entries whose `m_team` is the deleted instance (pointer
    /// identity). Name match is fallback for legacy/xfer entries without handle.
    pub fn ai_pre_team_destroy(&mut self, deleted: &Arc<RwLock<crate::team::Team>>) {
        let deleted_name = deleted.read().ok().map(|g| g.get_name().to_string());
        let keep = |q: &TeamInQueue| -> bool {
            if let Some(ref qt) = q.team {
                // C++: team->m_team == deletedTeam
                return !Arc::ptr_eq(qt, deleted);
            }
            // Fallback: name compare when m_team missing.
            if let (Some(ref dn), Some(ref qn)) = (deleted_name.as_ref(), q.team_name.as_ref()) {
                return qn != dn;
            }
            true
        };
        self.team_build_queue.retain(keep);
        self.team_ready_queue.retain(keep);
    }

    /// Name-based wrapper for call sites that only have a team name.
    pub fn ai_pre_team_destroy_by_name(&mut self, team_name: &str) {
        self.team_build_queue.retain(|team| {
            team.team_name
                .as_deref()
                .map(|name| name != team_name)
                .unwrap_or(true)
                && team
                    .team
                    .as_ref()
                    .and_then(|a| a.read().ok())
                    .map(|g| g.get_name().as_str() != team_name)
                    .unwrap_or(true)
        });
        self.team_ready_queue.retain(|team| {
            team.team_name
                .as_deref()
                .map(|name| name != team_name)
                .unwrap_or(true)
                && team
                    .team
                    .as_ref()
                    .and_then(|a| a.read().ok())
                    .map(|g| g.get_name().as_str() != team_name)
                    .unwrap_or(true)
        });
    }

    /// C++ `AIPlayer::guardSupplyCenter` (AIPlayer.cpp).
    ///
    /// Force attack check; prefer attacked center else findSupplyCenter; issue
    /// aiGuardPosition toward enemy base offset by warehouse radius*0.8.
    pub fn guard_supply_center(
        &mut self,
        team_name: &str,
        min_supplies: i32,
    ) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.supply_source_attack_check_frame = 0; // force check
        let mut warehouse_id = None;
        if self.is_supply_source_attacked() {
            warehouse_id = self.attacked_supply_center;
        }
        if warehouse_id.is_none() {
            warehouse_id = self
                .find_supply_center(min_supplies)
                .and_then(|w| w.read().ok().map(|g| g.get_id()));
        }
        let Some(warehouse_id) = warehouse_id else {
            return Ok(());
        };
        let Some((mut location, radius)) = OBJECT_REGISTRY.with_object(warehouse_id, |warehouse| {
            (
                *warehouse.get_position(),
                warehouse.get_geometry_info().get_bounding_circle_radius() * 0.8,
            )
        }) else {
            return Ok(());
        };

        // Offset toward enemy structure bounds center.
        let enemy_ndx = self.get_skirmish_enemy_player_index();
        if let Ok((lo, hi)) = self.get_player_structure_bounds(enemy_ndx) {
            let mut ox = location.x - (lo.x + hi.x) * 0.5;
            let mut oy = location.y - (lo.y + hi.y) * 0.5;
            let len = (ox * ox + oy * oy).sqrt();
            if len > 0.0001 {
                ox /= len;
                oy /= len;
                location.x -= ox * radius;
                location.y -= oy * radius;
            }
        }

        // Resolve team members (named team or default).
        let members: Vec<ObjectID> = {
            let mut team_arc = None;
            if !team_name.is_empty() {
                if let Ok(mut factory) = get_team_factory().lock() {
                    team_arc = factory.find_team(team_name);
                }
            }
            if team_arc.is_none() {
                if let Ok(list) = player_list().read() {
                    if let Some(player_arc) = list.get_player(self.player_id as i32) {
                        if let Ok(pg) = player_arc.read() {
                            team_arc = pg.get_default_team();
                        }
                    }
                }
            }
            team_arc
                .and_then(|t| t.read().ok().map(|g| g.get_members().to_vec()))
                .unwrap_or_default()
        };

        // C++: AIGroup::groupGuardPosition(&location, GUARDMODE_NORMAL, CMD_FROM_SCRIPT)
        // Issue per-member guard with script command source (not the no-op trait stub).
        for member_id in members {
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(member_id, |obj_g| obj_g.get_ai_update_interface())
                .flatten()
            {
                // AIUpdateInterfaceExt::ai_guard_position(pos, mode, cmd_source)
                ai.ai_guard_position(&location, GuardMode::Normal, CommandSourceType::FromScript);
            }
        }
        Ok(())
    }

    /// C++ `TheScriptEngine->getSkirmishEnemyPlayer()->getPlayerIndex()`.
    /// Prefer this player's current enemy, then first human, then any non-neutral.
    pub(super) fn get_skirmish_enemy_player_index(&self) -> i32 {
        if let Ok(list) = player_list().read() {
            if let Some(me) = list.get_player(self.player_id as i32) {
                if let Ok(mg) = me.read() {
                    if let Some(enemy_index) = mg.get_current_enemy_player_index() {
                        if let Some(enemy) = list.get_player(enemy_index) {
                            if let Ok(eg) = enemy.read() {
                                if eg.get_player_type() != PlayerType::Neutral {
                                    return enemy_index;
                                }
                            }
                        }
                    }
                }
            }
            // C++ ScriptEngine residual: first human player.
            for i in 0..list.get_player_count() {
                if let Some(p) = list.get_player(i as i32) {
                    if let Ok(pg) = p.read() {
                        if pg.get_player_type() == PlayerType::Human {
                            return i as i32;
                        }
                    }
                }
            }
            for i in 0..list.get_player_count() {
                let i = i as i32;
                if i == self.player_id as i32 {
                    continue;
                }
                if let Some(p) = list.get_player(i) {
                    if let Ok(pg) = p.read() {
                        if pg.get_player_type() != PlayerType::Neutral {
                            return i;
                        }
                    }
                }
            }
        }
        0
    }

    /// Get player structure bounds for targeting.
    /// Matches C++ `AIPlayer::getPlayerStructureBounds(bounds, playerNdx)` with
    /// `conservative = false` (default call sites).
    pub fn get_player_structure_bounds(
        &self,
        player_index: i32,
    ) -> Result<(Coord3D, Coord3D), AiError> {
        self.get_player_structure_bounds_ex(player_index, false)
    }

    /// C++ `AIPlayer::getPlayerStructureBounds(bounds, playerNdx, conservative)`.
    ///
    /// Structure AABB only (non-structures never contribute). When `conservative`,
    /// skip KINDOF_CONSERVATIVE_BUILDING. No structures → zeroed bounds (C++ leaves
    /// Region2D at 0). Final C++ `if (!firstStructure) *bounds = objBounds` is a
    /// no-op because both AABBs only track structures.
    pub fn get_player_structure_bounds_ex(
        &self,
        player_index: i32,
        conservative: bool,
    ) -> Result<(Coord3D, Coord3D), AiError> {
        // Wave 255: empty dual-world → empty bounds.
        if dual_world_registry_unavailable() {
            return Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)));
        }

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned())
        else {
            return Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)));
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)));
        };

        let mut first_structure = true;
        let mut struct_min = Coord3D::new(0.0, 0.0, 0.0);
        let mut struct_max = Coord3D::new(0.0, 0.0, 0.0);

        for obj_id in player_guard.get_all_objects() {
            let Some(pos) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    // C++ only enters the AABB expand when isKindOf(STRUCTURE).
                    if !obj_guard.is_kind_of(KindOf::Structure) {
                        return None;
                    }
                    // C++: conservative && KINDOF_CONSERVATIVE_BUILDING → skip.
                    if conservative && obj_guard.is_kind_of(KindOf::ConservativeBuilding) {
                        return None;
                    }
                    Some(*obj_guard.get_position())
                })
                .flatten()
            else {
                continue;
            };
            if first_structure {
                struct_min = Coord3D::new(pos.x, pos.y, pos.z);
                struct_max = Coord3D::new(pos.x, pos.y, pos.z);
                first_structure = false;
            } else {
                struct_min.x = struct_min.x.min(pos.x);
                struct_min.y = struct_min.y.min(pos.y);
                struct_max.x = struct_max.x.max(pos.x);
                struct_max.y = struct_max.y.max(pos.y);
            }
        }

        // No structures → zeroed bounds (C++ never copies unit-only bounds).
        if first_structure {
            Ok((Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0)))
        } else {
            Ok((struct_min, struct_max))
        }
    }

    /// Calculate center and radius of AI base
    /// Matches C++ AIPlayer computeCenterAndRadiusOfBase logic
    /// C++ `AIPlayer::computeCenterAndRadiusOfBase` (AIPlayer.cpp).
    ///
    /// Average of build-list entry locations (not live structures). Radius is
    /// max |dx|+geom*0.4 / |dy|+geom*0.4 Manhattan-as-axis-abs then hypot.
    pub fn compute_center_and_radius_of_base(&mut self) -> Result<(), AiError> {
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        // Pass 1: centroid of valid build-list locations.
        let mut entries: Vec<(f32, f32, f32)> = Vec::new();
        let mut cur = player_guard.get_build_list();
        while let Some(info) = cur {
            let name = info.get_template_name().to_string();
            if name.is_empty() {
                cur = info.get_next();
                continue;
            }
            let Some(template) = TheThingFactory::find_template(&name) else {
                cur = info.get_next();
                continue;
            };
            let pos = *info.get_location();
            let geom_r = template
                .get_template_geometry_info()
                .get_bounding_circle_radius();
            entries.push((pos.x, pos.y, geom_r));
            cur = info.get_next();
        }

        // leftover helper: centroid + |dx|+geom*0.4 hypot into max_rad_sqr
        let (set, cx, cy, radius) = leftover_compute_center_and_radius_of_base(&entries);
        self.base_center_set = set;
        if set {
            self.base_center = Coord3D::new(cx, cy, 0.0);
            self.base_radius = radius;
        } else {
            self.base_center = Coord3D::new(0.0, 0.0, 0.0);
            self.base_radius = 0.0;
        }
        Ok(())
    }

    pub(super) fn select_current_enemy_player(
        &self,
    ) -> Result<Option<(Arc<RwLock<Player>>, i32)>, AiError> {
        let Ok(list) = player_list().read() else {
            return Ok(None);
        };
        let Some(me_arc) = list.get_player(self.player_id as i32) else {
            return Ok(None);
        };
        let Ok(me_guard) = me_arc.read() else {
            return Ok(None);
        };
        if let Some(enemy_index) = me_guard.get_current_enemy_player_index() {
            if let Some(enemy_arc) = list.get_player(enemy_index).cloned() {
                let is_non_neutral = if let Ok(enemy_guard) = enemy_arc.read() {
                    enemy_guard.get_player_type() != PlayerType::Neutral
                } else {
                    false
                };
                if is_non_neutral {
                    return Ok(Some((enemy_arc, enemy_index)));
                }
            }
        }

        for (index, player_arc) in list.iter().enumerate() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() == PlayerType::Neutral {
                continue;
            }
            if player_guard.get_id() == self.player_id as i32 {
                continue;
            }
            return Ok(Some((player_arc.clone(), index as i32)));
        }

        Ok(None)
    }

    pub(super) fn count_active_harvesters(&self) -> usize {
        // Wave 255: empty dual-world → zero.

        if dual_world_registry_unavailable() {
            return 0;
        }

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
        else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };
        let mut count = 0;
        for obj_id in player_guard.get_all_objects() {
            let is_harvester = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| obj_guard.is_kind_of(KindOf::Harvester))
                .unwrap_or(false);
            if is_harvester {
                count += 1;
            }
        }
        count
    }

    /// C++ `AIPlayer::getPlayerSuperweaponValue` (AIPlayer.cpp).
    pub(super) fn get_player_superweapon_value(
        &self,
        center: &Coord3D,
        player_index: i32,
        radius: Real,
        include_military_units: bool,
    ) -> Result<i32, AiError> {
        // Wave 255: empty dual-world → Ok(0).
        if dual_world_registry_unavailable() {
            return Ok(0);
        }

        let radius = radius.max(4.0 * PATHFIND_CELL_SIZE_F);
        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned())
        else {
            return Ok(0);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(0);
        };

        let mut cash = 0.0_f32;
        let rad_sqr = radius * radius;
        for obj_id in player_guard.get_all_objects() {
            let Some((apply_neg_value, pos, template, is_cc, is_sw)) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    let mut apply_neg_value = false;
                    if !include_military_units {
                        // Sneak attack: defenses + combat units are hostile (negative).
                        if obj_guard.is_kind_of(KindOf::FSBaseDefense)
                            || obj_guard.is_kind_of(KindOf::TechBaseDefense)
                        {
                            apply_neg_value = true;
                        } else if (obj_guard.is_kind_of(KindOf::Vehicle)
                            || obj_guard.is_kind_of(KindOf::Infantry))
                            && !obj_guard.is_kind_of(KindOf::Dozer)
                            && !obj_guard.is_kind_of(KindOf::Harvester)
                        {
                            apply_neg_value = true;
                        }
                    } else if obj_guard.is_kind_of(KindOf::Aircraft)
                        && obj_guard.is_significantly_above_terrain()
                    {
                        // Only when valuing military: skip flying aircraft.
                        return None;
                    }

                    Some((
                        apply_neg_value,
                        *obj_guard.get_position(),
                        obj_guard.get_template().clone(),
                        obj_guard.is_kind_of(KindOf::CommandCenter),
                        obj_guard.is_kind_of(KindOf::FSSuperweapon),
                    ))
                })
                .flatten()
            else {
                continue;
            };

            let dx = center.x - pos.x;
            let dy = center.y - pos.y;
            if dx * dx + dy * dy >= rad_sqr {
                continue;
            }
            let dist = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - (dist / (2.0 * radius)); // 1.0 center, 0.5 edge
            // C++ calcCostToBuild(pPlayer) — pass player when possible.
            let mut value = template
                .calc_cost_to_build(Some(&*player_guard as &dyn std::any::Any))
                .max(0) as f32;
            if is_cc {
                value = if include_military_units {
                    value / 10.0
                } else {
                    value * 5.0
                };
            }
            if is_sw {
                value = if include_military_units {
                    value / 10.0
                } else {
                    value * 5.0
                };
            }
            if apply_neg_value {
                cash -= factor * value * 5.0;
            } else {
                cash += factor * value;
            }
        }
        // C++ returns Int (truncates Real cash).
        Ok(cash as i32)
    }
}
