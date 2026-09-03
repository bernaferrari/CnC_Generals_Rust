//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    /// C++ `AIPlayer::doBaseBuilding` (AIPlayer.cpp).
    ///
    /// structureTimer → readyToBuildStructure; buildDelay throttles processBaseBuilding
    /// to every `BUILD_DELAY_RECHECK_FRAMES` (2s), shortcut when structure completes.
    pub(super) fn do_base_building(&mut self) -> Result<(), AiError> {
        let can_build_base = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_can_build_base()))
            .unwrap_or(true);
        if !can_build_base {
            return Ok(());
        }

        // See if we are ready to start trying a structure.
        // C++ AIPlayer::doBaseBuilding has NO 3s clamp (only AISkirmishPlayer does).
        if !self.ready_to_build_structure {
            if self.structure_timer > 0 {
                self.structure_timer -= 1;
            }
            if self.structure_timer == 0 {
                self.ready_to_build_structure = true;
                self.build_delay = 0; // Cause immediate check
            }
        }

        // Throttle processBaseBuilding (C++ m_buildDelay).
        if self.build_delay > 0 {
            self.build_delay -= 1;
        }
        if self.build_delay == 0 {
            if self.ready_to_build_structure {
                self.process_base_building()?;
            }
            // processBaseBuilding may reset m_buildDelay (C++); only default if still 0.
            if self.build_delay == 0 {
                self.build_delay = BUILD_DELAY_RECHECK_FRAMES;
            }
        }

        Ok(())
    }

    pub(super) fn object_ai_is_idle(object_id: ObjectID) -> bool {
        // Wave 255: empty dual-world → fail-closed.

        if dual_world_registry_unavailable() {
            return false;
        }

        OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                obj.get_ai_update_interface()
                    .map(|ai| ai.is_idle())
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub(super) fn team_any_member_idle(team_name: &str) -> bool {
        let Ok(factory) = get_team_factory().lock() else {
            return false;
        };
        let Some(team_arc) = factory.find_team_instances(team_name).into_iter().next() else {
            return false;
        };
        drop(factory);
        let Ok(team) = team_arc.read() else {
            return false;
        };
        team.get_members()
            .iter()
            .copied()
            .any(Self::object_ai_is_idle)
    }

    pub(super) fn team_all_members_idle(team_name: &str) -> bool {
        let Ok(factory) = get_team_factory().lock() else {
            return true;
        };
        let Some(team_arc) = factory.find_team_instances(team_name).into_iter().next() else {
            return true;
        };
        drop(factory);
        let Ok(team) = team_arc.read() else {
            return true;
        };
        team.is_idle()
    }

    /// C++ `AIPlayer::checkReadyTeams` (AIPlayer.cpp).
    ///
    /// Activates ready-queue teams when all members are idle, any member is idle
    /// with an execute-actions production script, or 60s since `frame_started`.
    pub(crate) fn check_ready_teams(&mut self) -> Result<(), AiError> {
        let now = TheGameLogic::get_frame();
        let mut i = 0;
        while i < self.team_ready_queue.len() {
            let should_activate = {
                let team_q = &self.team_ready_queue[i];
                let time_expired = team_q
                    .frame_started
                    .saturating_add(60 * LOGICFRAMES_PER_SECOND)
                    < now;

                let (mut all_idle, mut any_idle) = (true, false);
                if team_q.reinforcement {
                    if let Some(obj_id) = team_q.reinforcement_id {
                        if let Some((idle,)) = OBJECT_REGISTRY
                            .with_object(obj_id, |obj| {
                                obj.get_ai_update_interface()
                                    .and_then(|ai| ai.lock().ok().map(|ai_g| (ai_g.is_idle(),)))
                            })
                            .flatten()
                        {
                            all_idle = idle;
                            any_idle = idle;
                        }
                    }
                } else if let Some(team_arc) = team_q.team.as_ref() {
                    // C++ team->m_team->isIdle() + member anyIdle walk.
                    if let Ok(tg) = team_arc.read() {
                        all_idle = tg.is_idle();
                        any_idle = false;
                        for mid in tg.get_members() {
                            if OBJECT_REGISTRY
                                .with_object(*mid, |og| {
                                    let Some(ai) = og.get_ai_update_interface() else {
                                        return false;
                                    };
                                    ai.lock().ok().map(|ai_g| ai_g.is_idle()).unwrap_or(false)
                                })
                                .unwrap_or(false)
                            {
                                any_idle = true;
                            }
                        }
                    }
                } else if let Some(team_name) = team_q.team_name.as_deref() {
                    // Fallback when m_team missing (legacy queue entries).
                    all_idle = Self::team_all_members_idle(team_name);
                    any_idle = Self::team_any_member_idle(team_name);
                }

                // C++: anyIdle && m_team->proto->m_executeActions &&
                // productionCondition script has Action → force allIdle.
                // Resolve prototype via concrete team name first, then team_name field.
                if any_idle {
                    let proto_name = team_q
                        .team
                        .as_ref()
                        .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
                        .or_else(|| team_q.team_name.clone());
                    if let Some(team_name) = proto_name {
                        if let Ok(factory) = get_team_factory().lock() {
                            if let Some(proto) = factory.find_team_prototype(&team_name) {
                                if proto.get_execute_actions_on_create() {
                                    let cond = proto.get_production_condition();
                                    if !cond.is_empty() {
                                        if let Ok(eng) = get_script_engine().read() {
                                            if eng
                                                .as_ref()
                                                .and_then(|e| {
                                                    e.find_script_clone_by_name(cond.as_str())
                                                })
                                                .and_then(|s| s.get_action().cloned())
                                                .is_some()
                                            {
                                                all_idle = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if time_expired {
                    all_idle = true;
                }
                all_idle
            };

            if !should_activate {
                i += 1;
                continue;
            }

            let mut team_q = self.team_ready_queue.remove(i).expect("ready idx");
            if !team_q.sent_to_start_location {
                team_q.sent_to_start_location = true;
                // C++ home-location tighten block is commented out in GeneralsMD.
            }

            if team_q.reinforcement {
                if let Some(obj_id) = team_q.reinforcement_id {
                    self.join_team_reinforcement(
                        obj_id,
                        team_q.team.clone(),
                        team_q.team_name.as_deref(),
                    );
                }
            } else {
                // C++ m_team->setActive() on the concrete team handle.
                if let Some(team_arc) = team_q.team.as_ref() {
                    if let Ok(mut tg) = team_arc.write() {
                        tg.set_active();
                    }
                } else if let Some(team_name) = team_q.team_name.as_deref() {
                    if let Ok(factory) = get_team_factory().lock() {
                        if let Some(team_arc) =
                            factory.find_team_instances(team_name).into_iter().next()
                        {
                            drop(factory);
                            if let Ok(mut tg) = team_arc.write() {
                                tg.set_active();
                            }
                        }
                    }
                }
                if self.is_skirmish_ai_player() {
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.clear_team_flags();
                        }
                    }
                }
            }
            // team_q dropped = C++ deleteInstance
        }

        Ok(())
    }

    /// C++ `AIUpdateInterface::joinTeam` for reinforcement activation.
    pub(super) fn join_team_reinforcement(
        &self,
        obj_id: ObjectID,
        _team: Option<Arc<RwLock<crate::team::Team>>>,
        _team_name: Option<&str>,
    ) {
        // Wave 255: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(obj_id, |obj| obj.get_ai_update_interface())
            .flatten()
        {
            // C++ joinTeam uses obj->getTeam(); team handle args are unused.
            ai.join_team();
        }
    }

    pub(super) fn is_skirmish_ai_player(&self) -> bool {
        player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.is_skirmish_ai()))
            .unwrap_or(false)
    }

    /// C++ `AIPlayer::checkQueuedTeams` (AIPlayer.cpp).
    ///
    /// 1. Expire build-time: min-built + complete → ready; else disband.
    /// 2. All-built → ready queue (prepend).
    /// 3. Any idle + executeActions → run productionCondition action (team-scoped).
    pub(crate) fn check_queued_teams(&mut self) -> Result<(), AiError> {
        // --- C++ phase 1: build-time expiry ---
        let mut i = 0;
        while i < self.team_build_queue.len() {
            let expired = self.team_build_queue[i].is_build_time_expired();
            if !expired {
                i += 1;
                continue;
            }
            let min_built = self.team_build_queue[i].is_minimum_built();
            if min_built {
                if self.team_build_queue[i].are_builds_complete() {
                    let team = self.team_build_queue.remove(i).expect("build idx");
                    // C++ prependTo_TeamReadyQueue
                    self.team_ready_queue.push_front(team);
                } else {
                    i += 1; // still building required units
                }
            } else {
                let mut team = self.team_build_queue.remove(i).expect("build idx");
                let _ = team.disband();
                if self.is_skirmish_ai_player() {
                    if let Ok(mut eng) = get_script_engine().write() {
                        if let Some(e) = eng.as_mut() {
                            e.clear_team_flags();
                        }
                    }
                }
            }
        }

        // --- C++ phase 2: all-built → ready; any-idle executeActions ---
        let mut i = 0;
        while i < self.team_build_queue.len() {
            if self.team_build_queue[i].is_all_built() {
                let team = self.team_build_queue.remove(i).expect("build idx");
                self.team_ready_queue.push_front(team);
                continue;
            }

            // anyIdle + executeActions → friend_executeAction(productionCondition)
            // C++ walks team->m_team members; prefer concrete handle.
            let any_idle = {
                let tq = &self.team_build_queue[i];
                if let Some(team_arc) = tq.team.as_ref() {
                    if let Ok(tg) = team_arc.read() {
                        let mut idle = false;
                        for mid in tg.get_members() {
                            let Some(ai) = OBJECT_REGISTRY
                                .with_object(*mid, |og| og.get_ai_update_interface())
                                .flatten()
                            else {
                                continue;
                            };
                            let Ok(aig) = ai.lock() else {
                                continue;
                            };
                            if aig.is_idle() {
                                idle = true;
                                break;
                            }
                        }
                        idle
                    } else {
                        false
                    }
                } else if let Some(ref name) = tq.team_name {
                    Self::team_any_member_idle(name)
                } else {
                    false
                }
            };

            if any_idle {
                // C++ uses team->m_team->getPrototype(); prefer handle name.
                let proto_name = self.team_build_queue[i]
                    .team
                    .as_ref()
                    .and_then(|arc| arc.read().ok().map(|tg| tg.get_name().to_string()))
                    .or_else(|| self.team_build_queue[i].team_name.clone());
                if let Some(ref name) = proto_name {
                    if let Ok(factory) = get_team_factory().lock() {
                        if let Some(proto) = factory.find_team_prototype(name) {
                            if proto.get_execute_actions_on_create() {
                                let cond = proto.get_production_condition().to_string();
                                drop(factory);
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
                                        // C++ friend_executeAction(action, team->m_team)
                                        drop(script_engine);
                                        if let Ok(mut eng) = get_script_engine().write() {
                                            if let Some(e) = eng.as_mut() {
                                                e.friend_execute_action(
                                                    &action,
                                                    Some(name.as_str()),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        // C++ checkQueuedTeams does not bind factories here — queueUnits does.

        Ok(())
    }

    /// C++ `AIPlayer::doTeamBuilding` (AIPlayer.cpp).
    ///
    /// teamTimer → readyToBuildTeam; teamDelay throttles queueUnits + processTeamBuilding
    /// to every `TEAM_DELAY_RECHECK_FRAMES` (5s), shortcut when unit/building completes.
    pub(super) fn do_team_building(&mut self) -> Result<(), AiError> {
        let can_build_units = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
            .and_then(|p| p.read().ok().map(|g| g.get_can_build_units()))
            .unwrap_or(true);
        if !can_build_units {
            return Ok(());
        }

        // See if we are ready to start trying a team.
        // C++ AIPlayer::doTeamBuilding has NO 3s clamp (only AISkirmishPlayer does).
        if !self.ready_to_build_team {
            if self.team_timer > 0 {
                self.team_timer -= 1;
            }
            if self.team_timer == 0 {
                self.ready_to_build_team = true;
                self.team_delay = 0; // Cause immediate check
            }
        }

        // Throttle queue/process (C++ m_teamDelay).
        if self.team_delay > 0 {
            self.team_delay -= 1;
        }
        if self.team_delay == 0 {
            // C++ always queueUnits on this cadence, then processTeamBuilding if ready.
            let _ = self.queue_units();
            if self.ready_to_build_team {
                self.process_team_building()?;
            }
            self.team_delay = TEAM_DELAY_RECHECK_FRAMES;
        }

        Ok(())
    }

    /// Process upgrades and skill purchases.
    /// Matches C++ AIPlayer::doUpgradesAndSkills() from AIPlayer.cpp:2906-2980.
    ///
    /// On first call, selects a skillset randomly from the available ones for the
    /// player's side. Then, if the player has science purchase points, iterates
    /// through the selected skillset and purchases each science that is affordable.
    pub(crate) fn do_upgrades_and_skills(&mut self) -> Result<(), AiError> {
        // C++ AIPlayer.cpp:2908-2910 — can't do updates on the first few frames.
        if TheGameLogic::get_frame() < 2 {
            return Ok(());
        }

        // C++: if (!getSciencePurchasePoints()) return; before sideInfo walk.
        let purchase_points_early = self
            .get_player()
            .and_then(|p| p.read().ok().map(|g| g.get_science_purchase_points()))
            .unwrap_or(0);
        if purchase_points_early <= 0 {
            return Ok(());
        }

        // Find the AiSideInfo for our player's side
        // C++ AIPlayer.cpp:2917-2926
        let player_side = {
            let Some(player_arc) = self.get_player() else {
                return Ok(());
            };
            let Ok(player_guard) = player_arc.read() else {
                return Ok(());
            };
            player_guard.get_side().clone()
        };

        // Get side info from AI data
        let ai_store = the_ai();let side_info = ai_store.read().ok().and_then(|ai_guard| {
            let ai_data = ai_guard.get_ai_data();
            let data = ai_data.read().ok()?;
            data.side_info
                .iter()
                .find(|info| info.side == player_side)
                .cloned()
        });

        let Some(side_info) = side_info else {
            return Ok(());
        };

        // Skillset selection: pick randomly among defined skillsets
        // C++ AIPlayer.cpp:2928-2948 (after science-points early-out).
        if self.skillset_selector == INVALID_SKILLSET_SELECTION {
            let mut limit: u32 = 0;
            // Pick randomly among the skillsets that have skills.
            // Designers sometimes only define skillset 1 & 2, or some such.
            if side_info.skill_set_2.num_skills > 0 {
                limit = 1;
                if side_info.skill_set_3.num_skills > 0 {
                    limit = 2;
                    if side_info.skill_set_4.num_skills > 0 {
                        limit = 3;
                        if side_info.skill_set_5.num_skills > 0 {
                            limit = 4;
                        }
                    }
                }
            }
            // C++ AIPlayer::isSkirmishAI() — false on base AIPlayer, true on skirmish.
            if self.is_skirmish_ai_player() {
                self.skillset_selector = game_logic_random_value(0, limit) as i32;
            } else {
                // Non-skirmish default to 0
                self.skillset_selector = 0;
            }
        }

        // SKILLS: purchase sciences from the selected skillset
        // C++ AIPlayer.cpp:2951-2977
        let Some(player_arc) = self.get_player() else {
            return Ok(());
        };
        let purchase_points = {
            let Ok(player_guard) = player_arc.read() else {
                return Ok(());
            };
            player_guard.get_science_purchase_points()
        };
        if purchase_points <= 0 {
            return Ok(());
        }

        let skillset: &crate::ai::SkillSet = match self.skillset_selector {
            0 => &side_info.skill_set_1,
            1 => &side_info.skill_set_2,
            2 => &side_info.skill_set_3,
            3 => &side_info.skill_set_4,
            _ => &side_info.skill_set_5,
        };

        // Attempt to purchase each science in the skillset
        for i in 0..skillset.num_skills as usize {
            if i >= skillset.skills.len() {
                break;
            }
            let science = skillset.skills[i];
            if science == crate::common::science::SCIENCE_INVALID {
                continue;
            }
            let (capable, purchased) = {
                let Ok(mut player_guard) = player_arc.write() else {
                    break;
                };
                let capable = player_guard.is_capable_of_purchasing_science(science);
                if !capable {
                    (false, false)
                } else {
                    let purchased = player_guard.attempt_to_purchase_science(science);
                    (true, purchased)
                }
            };
            if capable && purchased {
                // Successfully purchased a science from the skillset
                log::debug!(
                    "AI Player purchases from SkillSet{} science {}",
                    self.skillset_selector + 1,
                    science,
                );
            }
        }

        Ok(())
    }

    /// C++ `AIPlayer::updateBridgeRepair` (AIPlayer.cpp).
    ///
    /// Once/second: pop dead queue heads, assign/find repair dozer, issue
    /// aiRepair, complete when pristine and idle, then send dozer home.
    pub(crate) fn update_bridge_repair(&mut self) -> Result<(), AiError> {
        use crate::ai::{AiCommandParams, AiCommandType};
        use crate::object::body::BodyDamageType;
        use crate::object::update::ai_update::dozer_ai_update::DozerTask;

        if self.structures_in_queue <= 0 {
            return Ok(());
        }
        // C++: m_bridgeTimer--; if (m_bridgeTimer>0) return; m_bridgeTimer = FPS;
        // Decrement first so timer==1 proceeds this frame (not FPS+1 lag).
        self.bridge_timer = self.bridge_timer.saturating_sub(1);
        if self.bridge_timer > 0 {
            return Ok(());
        }
        self.bridge_timer = LOGICFRAMES_PER_SECOND;

        // Pop missing heads.
        let mut bridge_id = None;
        while bridge_id.is_none() && self.structures_in_queue > 0 {
            let head = self.structures_to_repair[0];
            if head
                .and_then(|id| OBJECT_REGISTRY.with_object(id, |_| ()))
                .is_some()
            {
                bridge_id = head;
            } else {
                // shift left
                for i in 0..(self.structures_in_queue as usize).saturating_sub(1) {
                    self.structures_to_repair[i] = self.structures_to_repair[i + 1];
                }
                if self.structures_in_queue > 0 {
                    let last = (self.structures_in_queue as usize) - 1;
                    self.structures_to_repair[last] = None;
                    self.structures_in_queue -= 1;
                }
            }
        }
        if self.structures_in_queue <= 0 {
            return Ok(());
        }
        let Some(bridge_id) = bridge_id else {
            return Ok(());
        };
        let Some((bridge_state, bridge_pos)) = OBJECT_REGISTRY.with_object(bridge_id, |bg| {
            let bridge_state = bg
                .get_body_module()
                .and_then(|b| b.lock().ok().map(|g| g.get_damage_state()))
                .unwrap_or(BodyDamageType::Pristine);
            (bridge_state, *bg.get_position())
        }) else {
            return Ok(());
        };

        if self.repair_dozer.is_none() {
            self.dozer_is_repairing = false;
            if self.dozer_queued_for_repair {
                return Ok(()); // waiting for queued dozer
            }
            if let Some(dozer_id) = self.find_dozer(&bridge_pos)? {
                self.repair_dozer = Some(dozer_id);
                if let Some(pos) = OBJECT_REGISTRY.with_object(dozer_id, |dg| *dg.get_position()) {
                    self.repair_dozer_origin = pos;
                }
                if let Some(ai) = OBJECT_REGISTRY
                    .with_object(dozer_id, |dg| dg.get_ai_update_interface())
                    .flatten()
                {
                    if let Ok(mut ai_lock) = ai.lock() {
                        let mut params =
                            AiCommandParams::new(AiCommandType::Repair, CommandSourceType::FromAi);
                        params.obj = Some(bridge_id);
                        let _ = ai_lock.execute_command(&params);
                    }
                }
                self.dozer_is_repairing = true;
                return Ok(());
            }
            self.queue_dozer()?;
            self.dozer_queued_for_repair = true;
            return Ok(());
        }

        let Some(dozer_id) = self.repair_dozer else {
            return Ok(());
        };
        let Some(ai) = OBJECT_REGISTRY.with_object(dozer_id, |dg| dg.get_ai_update_interface())
        else {
            self.repair_dozer = None; // killed
            self.bridge_timer = 0;
            return Ok(());
        };
        let Some(ai) = ai else {
            return Ok(());
        };

        let any_task_pending = {
            let Ok(mut ai_g) = ai.lock() else {
                return Ok(());
            };
            ai_g.get_dozer_ai_update_interface_mut()
                .map(|d| d.is_any_task_pending())
                .unwrap_or(false)
        };

        if self.dozer_is_repairing {
            if !any_task_pending {
                if bridge_state == BodyDamageType::Pristine {
                    // Done — pop head.
                    for i in 0..(self.structures_in_queue as usize).saturating_sub(1) {
                        self.structures_to_repair[i] = self.structures_to_repair[i + 1];
                    }
                    if self.structures_in_queue > 0 {
                        let last = (self.structures_in_queue as usize) - 1;
                        self.structures_to_repair[last] = None;
                        self.structures_in_queue -= 1;
                    }
                    self.dozer_is_repairing = false;
                    if self.structures_in_queue == 0 {
                        // Go home to base center or origin.
                        // C++: pathfinder->adjustToPossibleDestination(dozer, locoSet, &pos)
                        // then aiMoveToPosition(&pos, CMD_FROM_AI).
                        let mut pos = if self.base_center_set {
                            self.base_center
                        } else {
                            self.repair_dozer_origin
                        };
                        if let Some((start, ai)) = OBJECT_REGISTRY
                            .with_object(dozer_id, |dg| {
                                dg.get_ai_update_interface()
                                    .map(|ai| (*dg.get_position(), ai))
                            })
                            .flatten()
                        {
                            // Adjust destination onto a reachable cell with dozer loco set.
                            if let Some(loco_set) = ai.get_locomotor_set_clone() {
                                let ai_store = the_ai(); if let Ok(ai_sys) = ai_store.read() {
                                    if let Some(pf_arc) = ai_sys.pathfinder() {
                                        if let Ok(pf) = pf_arc.read() {
                                            let surfaces = loco_set.get_valid_surfaces();
                                            let _ = pf.adjust_to_possible_destination(
                                                &start, &mut pos, surfaces, false, 0.0,
                                            );
                                        }
                                    }
                                }
                            }
                            ai.ai_move_to_position(&pos, false, CommandSourceType::FromAi);
                        }
                        return Ok(());
                    }
                }
            } else {
                return Ok(()); // still working
            }
        }

        // (Re)issue repair.
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(dozer_id, |dg| dg.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_lock) = ai.lock() {
                let mut params =
                    AiCommandParams::new(AiCommandType::Repair, CommandSourceType::FromAi);
                params.obj = Some(bridge_id);
                let _ = ai_lock.execute_command(&params);
            }
        }
        self.dozer_is_repairing = true;
        let _ = DozerTask::Build; // keep import path warm if needed
        Ok(())
    }
}
