//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// C++ `AIPlayer::queueUnits` (AIPlayer.cpp).
    ///
    /// For each work order still waiting: recruit existing map units into the
    /// team (tryToRecruit) until full or none left; then startTraining if still
    /// waiting; else validateFactory.
    pub fn queue_units(&mut self) -> bool {
        let _ = self.queue_supply_truck();

        let ai_store = the_ai();let max_recruit = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.max_recruit_distance))
            .filter(|d| *d > 0.0)
            .unwrap_or(99999.0);

        let mut rebuilt_queue = VecDeque::with_capacity(self.team_build_queue.len());
        while let Some(mut team_q) = self.team_build_queue.pop_front() {
            let busy_ok = team_q.priority_build;
            let team_name = team_q
                .team_name
                .clone()
                .unwrap_or_else(|| "default".to_string());

            // C++ team->m_team: prefer concrete handle; name lookup is fallback only.
            if team_q.team.is_none() {
                team_q.team = get_team_factory().lock().ok().and_then(|mut factory| {
                    factory
                        .find_team_instances(&team_name)
                        .into_iter()
                        .next()
                        .or_else(|| factory.find_team(&team_name))
                });
            }
            let team_arc = team_q.team.clone();

            // Home for recruit search: C++ m_team prototype homeLocation else base center.
            let (home, has_home) = self.queue_units_home_for_team(team_arc.as_ref(), &team_name);

            for order in &mut team_q.work_orders {
                // C++: while waiting, tryToRecruit repeatedly.
                if let Some(ref team_arc) = team_arc {
                    while order.is_waiting_to_build() {
                        let Some(thing) = TheThingFactory::find_template(&order.thing_template)
                        else {
                            break;
                        };
                        let Ok(team_g) = team_arc.read() else {
                            break;
                        };
                        let Some(unit_arc) = team_g.try_to_recruit(&thing, &home, max_recruit)
                        else {
                            break; // no more recruitable units
                        };
                        drop(team_g);

                        order.num_completed = order.num_completed.saturating_add(1);

                        if let Ok(mut unit_g) = unit_arc.write() {
                            let _ = unit_g.set_team(Some(team_arc.clone()));
                            if let Some(ai) = unit_g.get_ai_update_interface() {
                                if has_home {
                                    // C++ aiMoveToPosition(&home, CMD_FROM_AI)
                                    ai.ai_move_to_position(&home, false, CommandSourceType::FromAi);
                                } else {
                                    // C++ aiIdle(CMD_FROM_AI)
                                    ai.ai_idle(CommandSourceType::FromAi);
                                }
                            }
                        }

                        log::debug!(
                            "Team '{}' recruits {} (queueUnits)",
                            team_name,
                            order.thing_template
                        );
                    }
                }

                if order.is_waiting_to_build() {
                    // start the creation of a new unit
                    // C++ startTraining(..., team->m_team->getName())
                    let train_name = team_arc
                        .as_ref()
                        .and_then(|a| a.read().ok().map(|g| g.get_name().to_string()))
                        .unwrap_or_else(|| team_name.clone());
                    let _ = self.start_training_internal(order, busy_ok, train_name.as_str());
                } else {
                    // under construction / complete — verify factory still exists
                    let _ = order.validate_factory(self.player_id);
                }
            }
            rebuilt_queue.push_back(team_q);
        }
        self.team_build_queue = rebuilt_queue;

        true
    }

    /// C++ queueUnits home: m_team prototype homeLocation if set, else getBaseCenter.
    pub(super) fn queue_units_home_for_team(
        &self,
        team: Option<&Arc<RwLock<crate::team::Team>>>,
        team_name: &str,
    ) -> (Coord3D, bool) {
        // Resolve prototype name from concrete m_team when present (C++ getPrototype()).
        let proto_name = team
            .and_then(|a| a.read().ok().map(|g| g.get_name().to_string()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| team_name.to_string());
        if let Ok(factory) = get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(&proto_name) {
                if proto.has_home_location() {
                    return (proto.home_location(), true);
                }
            }
            // Fallback: original team_name if m_team name differs.
            if proto_name != team_name {
                if let Some(proto) = factory.find_team_prototype(team_name) {
                    if proto.has_home_location() {
                        return (proto.home_location(), true);
                    }
                }
            }
        }
        // C++ falls back to base center when !hasHomeLocation.
        if let Some(center) = self.get_base_center() {
            return (center, false);
        }
        (Coord3D::new(0.0, 0.0, 0.0), false)
    }

    /// C++ onUnitProduced supply assignment: first build-list supply building with
    /// desiredGatherers > currentGatherers; bump current and return object id.
    pub(super) fn take_supply_gatherer_slot(&mut self) -> Option<ObjectID> {
        // Wave 255: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let player_arc = self.get_player()?;
        let Ok(mut pg) = player_arc.write() else {
            return None;
        };
        let Some(info_head) = pg.get_build_list_mut() else {
            return None;
        };
        let mut node = Some(&mut *info_head);
        while let Some(info) = node {
            if info.is_supply_building()
                && info.get_desired_gatherers() > 0
                && info.get_desired_gatherers() > info.get_current_gatherers()
            {
                let oid = info.get_object_id();
                if oid != INVALID_ID && OBJECT_REGISTRY.with_object(oid, |_| ()).is_some() {
                    info.set_current_gatherers(info.get_current_gatherers() + 1);
                    return Some(oid);
                }
            }
            node = info.get_next_mut();
        }
        None
    }

    /// C++ `AIPlayer::checkForSupplyCenter` (AIPlayer.cpp).
    ///
    /// If structure has SupplyCenterDockUpdate, mark build-list entry as supply
    /// building and set desired gatherers from AISideInfo + 1 freebie.
    pub fn check_for_supply_center(&mut self, structure_id: ObjectID) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let has_dock = OBJECT_REGISTRY
            .with_object(structure_id, |structure_guard| {
                // C++: findUpdateModule(NAMEKEY("SupplyCenterDockUpdate")) only —
                // KindOf alone is not sufficient (matches GeneralsMD AIPlayer.cpp).
                structure_guard
                    .find_update_module("SupplyCenterDockUpdate")
                    .is_some()
            })
            .unwrap_or(false);
        if !has_dock {
            return Ok(());
        }

        let side = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_side().to_string()))
            .unwrap_or_default();

        let mut desired = 0;
        let ai_store = the_ai(); if let Ok(ai_guard) = ai_store.read() {
            if let Ok(ai_data) = ai_guard.get_ai_data().read() {
                for info in &ai_data.side_info {
                    if info.side == side {
                        desired = match self.difficulty {
                            GameDifficulty::Easy => info.easy,
                            GameDifficulty::Normal => info.normal,
                            GameDifficulty::Hard | GameDifficulty::Brutal => info.hard,
                        };
                        break;
                    }
                }
            }
        }

        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(mut pg) = player_arc.write() {
                    if let Some(info) = pg.get_build_list_mut() {
                        let mut cur = Some(&mut *info);
                        while let Some(node) = cur {
                            if node.get_object_id() == structure_id {
                                node.set_supply_building(true);
                                node.set_current_gatherers(-1);
                                // C++ desiredGatherers + 1 freebie with depot
                                node.set_desired_gatherers(desired + 1);
                                break;
                            }
                            cur = node.get_next_mut();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn select_team_to_build_ai(&mut self) -> bool {
        self.select_team_to_build().unwrap_or(false)
    }

    /// C++ `AIPlayer::setAIDifficulty` — assign `m_difficulty` only.
    ///
    /// Does not rewrite TeamSeconds or host strategy factors (those are not in
    /// GeneralsMD AIPlayer::setAIDifficulty).
    pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    /// C++ `AIPlayer::selectSkillset` — assign skillset; warn if already chosen.
    pub fn select_skillset(&mut self, skillset: i32) {
        if self.skillset_selector != INVALID_SKILLSET_SELECTION {
            log::debug!(
                "Selecting a skill set ({}) after one has already been chosen ({}) means some points have been incorrectly spent.",
                skillset + 1,
                self.skillset_selector + 1
            );
        }
        self.skillset_selector = skillset;
    }

    /// C++ `AIPlayer::processTeamBuilding`: if selectTeamToBuild then queueUnits.
    /// (selectTeamToBuild itself may reinforce a higher-priority team first.)
    pub fn process_team_building(&mut self) -> Result<(), AiError> {
        if self.select_team_to_build()? {
            let _ = self.queue_units();
        }
        Ok(())
    }

    /// C++ `AIPlayer::isSupplySourceSafe` (AIPlayer.cpp).
    pub fn is_supply_source_safe(&self, min_supplies: i32) -> bool {
        let Some(warehouse) = self.find_supply_center(min_supplies) else {
            return true; // safe because it doesn't exist
        };
        let Ok(guard) = warehouse.read() else {
            return true;
        };
        let template = guard.get_template();
        self.is_location_safe(guard.get_position(), template.as_ref())
    }

    /// C++ `AIPlayer::isSupplySourceAttacked` (AIPlayer.cpp).
    ///
    /// Rate-limited (10s): if player was recently attacked, scan cash generators /
    /// dozers / harvesters for recent damage and latch attacked_supply_center.
    pub fn is_supply_source_attacked(&mut self) -> bool {
        // Wave 255: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        // C++ AIPlayer.cpp: const Int SCAN_RATE = 10;
        // Comment says "10 seconds" but the value is added to frame counters as-is
        // (10 logic frames ≈ 0.33s). Match code, not the misleading comment.
        const SCAN_RATE: u32 = 10;
        let cur_frame = TheGameLogic::get_frame();
        if cur_frame == 0 {
            self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);
            return false;
        }
        self.attacked_supply_center = None;
        if cur_frame < self.supply_source_attack_check_frame {
            return false;
        }

        let Ok(list) = player_list().read() else {
            return false;
        };
        let Some(player_arc) = list.get_player(self.player_id as i32) else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        if player_guard.get_attacked_frame().saturating_add(SCAN_RATE) < cur_frame {
            return false; // haven't been attacked recently
        }
        self.supply_source_attack_check_frame = cur_frame.saturating_add(SCAN_RATE);

        for obj_id in player_guard.get_all_objects() {
            let Some(body) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if !obj_guard.is_kind_of(KindOf::CashGenerator)
                        && !obj_guard.is_kind_of(KindOf::Dozer)
                        && !obj_guard.is_kind_of(KindOf::Harvester)
                    {
                        return None;
                    }
                    obj_guard.get_body_module()
                })
                .flatten()
            else {
                continue;
            };
            let Ok(body_g) = body.lock() else {
                continue;
            };
            let Some(info) = body_g.get_last_damage_info() else {
                continue;
            };
            if info.output.no_effect {
                continue;
            }
            if body_g.get_last_damage_timestamp().saturating_add(SCAN_RATE) > cur_frame {
                self.attacked_supply_center = Some(obj_id);
                return true;
            }
        }
        false
    }

    /// C++ `AIPlayer::buildSpecificAITeam` (AIPlayer.cpp).
    ///
    /// Gates: canBuildUnits, singleton+priority, isPossibleToBuildTeam (money-
    /// only still queues). Work orders: optional (max-min) then required (min,
    /// even minUnits==0). createInactiveTeam, executeActions, priority prepend
    /// vs normal append, teamDelay=0.
    pub fn build_specific_ai_team(
        &mut self,
        team_name: &str,
        priority_build: bool,
    ) -> Result<(), AiError> {
        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };
        if !player_guard.get_can_build_units() {
            log::debug!(
                "Can't build team '{}' because build units is disabled.",
                team_name
            );
            return Ok(());
        }
        drop(player_guard);

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(proto) = factory.find_team_prototype(team_name).map(|p| p.clone()) else {
            return Ok(());
        };

        if priority_build && proto.is_singleton() {
            if let Some(existing) = factory.find_team(team_name) {
                if let Ok(eg) = existing.read() {
                    if eg.has_any_objects() {
                        log::debug!(
                            "Unable to build singleton team '{}' because team already exists.",
                            team_name
                        );
                        return Ok(());
                    }
                }
            }
        }

        // Drop factory lock before is_possible (find_factory may lock).
        let units: Vec<(String, i32, i32)> = proto
            .units_info()
            .iter()
            .filter(|u| !u.unit_thing_name.is_empty())
            .map(|u| (u.unit_thing_name.to_string(), u.min_units, u.max_units))
            .collect();
        drop(factory);

        let (possible, need_money) = self.is_possible_to_build_team(team_name, false)?;
        if !possible {
            if need_money {
                log::debug!(
                    "Note - queueing team '{}' but there is not enough money.",
                    team_name
                );
                // C++ still queues when only money is missing.
            } else {
                log::debug!(
                    "Unable to build team '{}' because required factories/tech don't exist.",
                    team_name
                );
                return Ok(());
            }
        }

        // Optional units first (max-min), then required (min) — C++ prepend order
        // so required ends up first in list after both prepends.
        // C++ still creates required WorkOrders when minUnits==0 (numRequired=0).
        let mut orders: Vec<WorkOrder> = Vec::new();
        // Optional
        for (name, min_u, max_u) in &units {
            let count = (*max_u - *min_u).max(0);
            if count <= 0 {
                continue;
            }
            if TheThingFactory::find_template(name).is_none() {
                continue;
            }
            let mut order = WorkOrder::new(name.clone());
            order.num_required = count;
            order.required = false;
            orders.insert(0, order); // prepend
        }
        // Required — always when template exists (even minUnits==0).
        for (name, min_u, _max_u) in &units {
            if TheThingFactory::find_template(name).is_none() {
                continue;
            }
            let count = (*min_u).max(0);
            let mut order = WorkOrder::new(name.clone());
            order.num_required = count;
            order.required = true;
            orders.insert(0, order); // prepend
        }

        if orders.is_empty() {
            log::debug!("{} - contains 0 buildable units.", team_name);
            return Ok(());
        }

        // createInactiveTeam
        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(team_arc) = factory.create_inactive_team(team_name) else {
            return Ok(());
        };
        drop(factory);

        if let Ok(mut tg) = team_arc.write() {
            tg.set_controlling_player_id(Some(self.player_id as UnsignedInt));
        }

        // C++: if executeActions, friend_executeAction(productionCondition action, team).
        if proto.get_execute_actions_on_create() {
            let cond = proto.get_production_condition().to_string();
            if !cond.is_empty() {
                let script_engine = get_script_engine();
                let action = script_engine
                    .read()
                    .ok()
                    .and_then(|eng| {
                        eng.as_ref()
                            .and_then(|e| e.find_script_clone_by_name(&cond))
                    })
                    .and_then(|script| script.get_action().cloned());
                if let Some(action) = action {
                    // C++ friend_executeAction(action, team)
                    drop(script_engine);
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.friend_execute_action(&action, Some(team_name));
                        }
                    }
                }
            }
        }

        let mut team = TeamInQueue::new();
        team.team_name = Some(team_name.to_string());
        team.team = Some(team_arc);
        team.priority_build = priority_build;
        team.frame_started = TheGameLogic::get_frame();
        team.work_orders = orders;

        if priority_build {
            self.team_build_queue.push_front(team);
        } else {
            self.team_build_queue.push_back(team);
        }
        self.team_delay = 0;
        log::debug!("{} - starting team build.", team_name);
        Ok(())
    }

    /// C++ `AIPlayer::buildAIBaseDefense` — solo AI unsupported (skirmish overrides).
    pub fn build_ai_base_defense(&mut self, _flank: bool) -> Result<(), AiError> {
        log::debug!("Error : Solo ai doesn't support buildAIBaseDefense.");
        Ok(())
    }

    /// C++ `AIPlayer::buildAIBaseDefenseStructure` — solo AI unsupported.
    pub fn build_ai_base_defense_structure(
        &mut self,
        _structure_name: &str,
        _flank: bool,
    ) -> Result<(), AiError> {
        log::debug!("Error : Solo ai doesn't support buildAIBaseDefenseStructure.");
        Ok(())
    }

    /// Build specific building as soon as possible
    /// C++ `AIPlayer::buildSpecificAIBuilding` — solo AI does not support this;
    /// skirmish override handles real priority-build stamping.
    pub fn build_specific_ai_building(&mut self, building_name: &str) -> Result<(), AiError> {
        log::debug!(
            "Error : Solo ai doesn't support BuildSpecificBuilding. '{}' not built.",
            building_name
        );
        Ok(())
    }

    /// C++ `AIPlayer::recruitSpecificAITeam` (AIPlayer.cpp).
    ///
    /// createInactiveTeam, tryToRecruit up to maxUnits per type within radius of
    /// home/base, move to home, ready-queue if any recruited else disband.
    pub fn recruit_specific_ai_team(
        &mut self,
        team_name: &str,
        recruit_radius: Real,
    ) -> Result<(), AiError> {
        let radius = if recruit_radius < 1.0 {
            99_999.0
        } else {
            recruit_radius
        };

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(());
        };
        let Some(proto) = factory.find_team_prototype(team_name).map(|p| p.clone()) else {
            return Ok(());
        };

        if proto.is_singleton() {
            if let Some(existing) = factory.find_team(team_name) {
                if let Ok(eg) = existing.read() {
                    if eg.has_any_objects() {
                        log::debug!(
                            "Unable to recruit singleton team '{}' because team already exists.",
                            team_name
                        );
                        return Ok(());
                    }
                }
            }
        }

        // C++: warn missing home when not skirmish AI (AIPlayer) / always for skirmish
        // override path. Still recruits using template home (often origin).
        if !proto.has_home_location() && !self.is_skirmish_ai_player() {
            log::debug!(
                "Error : team '{}' has no Home Position (or Origin).",
                team_name
            );
        }

        let Some(team_arc) = factory.create_inactive_team(team_name) else {
            return Ok(());
        };
        drop(factory);

        if let Ok(mut tg) = team_arc.write() {
            tg.set_controlling_player_id(Some(self.player_id as UnsignedInt));
        }

        // C++ tryToRecruit / aiMoveToPosition use teamProto homeLocation.
        let home = proto.home_location();

        let mut units_recruited = 0i32;
        for unit_info in proto.units_info() {
            if unit_info.unit_thing_name.is_empty() {
                continue;
            }
            let Some(thing) = TheThingFactory::find_template(unit_info.unit_thing_name) else {
                continue;
            };
            let mut count = unit_info.max_units.max(0);
            while count > 0 {
                let recruited = {
                    let Ok(tg) = team_arc.read() else {
                        break;
                    };
                    tg.try_to_recruit(&thing, &home, radius)
                };
                let Some(unit_arc) = recruited else {
                    break;
                };
                let unit_id = unit_arc
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(INVALID_ID);
                if let Ok(mut ug) = unit_arc.write() {
                    let _ = ug.set_team(Some(team_arc.clone()));
                }
                if let Ok(mut tg) = team_arc.write() {
                    tg.add_member(unit_id);
                }
                // Move to home (CMD_FROM_AI).
                if let Ok(ug) = unit_arc.read() {
                    if let Some(ai) = ug.get_ai_update_interface() {
                        if let Ok(mut ai_g) = ai.lock() {
                            let mut params = crate::ai::AiCommandParams::new(
                                crate::ai::AiCommandType::MoveToPosition,
                                CommandSourceType::FromAi,
                            );
                            params.pos = home;
                            let _ = ai_g.execute_command(&params);
                        }
                    }
                }
                units_recruited += 1;
                count -= 1;
            }
        }

        if units_recruited > 0 {
            let mut team = TeamInQueue::new();
            team.team_name = Some(team_name.to_string());
            team.team = Some(team_arc);
            team.priority_build = false;
            team.frame_started = TheGameLogic::get_frame();
            // Ready queue — C++ prependTo_TeamReadyQueue (activate later).
            self.team_ready_queue.push_front(team);
            log::debug!("{} - Finished recruiting.", team_name);
        } else {
            if !proto.is_singleton() {
                let team_id = team_arc.read().ok().map(|t| t.get_id());
                if let (Some(team_id), Ok(mut factory)) = (team_id, get_team_factory().lock()) {
                    factory.team_about_to_be_deleted(team_id);
                }
            }
            log::debug!("{} - Recruited 0 units, disbanding.", team_name);
        }

        Ok(())
    }
}
