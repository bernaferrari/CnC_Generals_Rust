//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

impl AIPlayer {
    pub(super) fn get_player_arc(&self) -> Option<Arc<RwLock<Player>>> {
        player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(self.player_id as i32).cloned())
    }

    /// Get the backing Player for this AI instance.
    pub fn get_player(&self) -> Option<Arc<RwLock<Player>>> {
        self.get_player_arc()
    }

    /// Create new AI player
    pub fn new(player_id: u32) -> Self {
        // C++ AIPlayer ctor: m_teamSeconds = TheAI->getAiData()->m_teamSeconds;
        // Structure interval is read live from AIData each arm (0.0 is valid = every tick).
        // Prefer live AIData; fall back to retail Default/AIData.ini constants when unloaded.
        let (team_seconds, structure_seconds) = if let Ok(ai) = THE_AI.read() {
            if let Ok(data) = ai.get_ai_data().read() {
                let team = if data.team_seconds > 0.0 {
                    data.team_seconds
                } else {
                    DEFAULT_TEAM_SECONDS
                };
                // StructureSeconds = 0.0 is intentional retail (do not treat as missing).
                (team, data.structure_seconds)
            } else {
                (DEFAULT_TEAM_SECONDS, DEFAULT_STRUCTURE_SECONDS)
            }
        } else {
            (DEFAULT_TEAM_SECONDS, DEFAULT_STRUCTURE_SECONDS)
        };

        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32).cloned() {
                if let Ok(mut player_guard) = player_arc.write() {
                    player_guard.set_can_build_units(false);
                }
            }
        }

        // C++ AIPlayer::AIPlayer: m_difficulty = TheScriptEngine->getGlobalDifficulty().
        let difficulty = get_script_engine()
            .read()
            .ok()
            .and_then(|eng| eng.as_ref().map(|e| e.get_global_difficulty()))
            .unwrap_or(GameDifficulty::Normal);

        Self {
            player_id,
            team_build_queue: VecDeque::new(),
            team_ready_queue: VecDeque::new(),
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 2,
            structure_timer: 2,
            team_seconds,
            structure_seconds,
            build_delay: 0,
            team_delay: 0,
            frame_last_building_built: TheGameLogic::get_frame(),
            difficulty,
            skillset_selector: INVALID_SKILLSET_SELECTION,
            base_center: Coord3D::new(0.0, 0.0, 0.0),
            base_center_set: false,
            base_radius: 0.0,
            structures_to_repair: [None; MAX_STRUCTURES_TO_REPAIR],
            repair_dozer: None,
            repair_dozer_origin: Coord3D::new(0.0, 0.0, 0.0),
            structures_in_queue: 0,
            dozer_queued_for_repair: false,
            dozer_is_repairing: false,
            bridge_timer: 0,
            supply_source_attack_check_frame: 0,
            attacked_supply_center: None,
            current_warehouse_id: None,
            strategy_state: AiStrategyState::default(),
            economic_state: AiEconomicState::default(),
            military_state: AiMilitaryState::default(),
            construction_priorities: Vec::new(),
            threat_assessment: ThreatAssessment::default(),
            strategic_decision_maker: StrategicDecisionMaker::new(),
            difficulty_handler: DifficultyHandler::new(to_ai_difficulty(difficulty), "USA"),
            build_order_optimizer: BuildOrderOptimizer::new(),
            threat_system: ThreatAssessmentSystem::new(),
        }
    }

    /// Get base center position
    pub fn get_base_center(&self) -> Option<Coord3D> {
        if self.base_center_set {
            Some(self.base_center)
        } else {
            None
        }
    }

    pub fn get_base_radius(&self) -> Real {
        self.base_radius
    }

    pub fn get_ai_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Public update entrypoint used by the integration layer.
    pub fn update(&mut self) -> Result<(), AiError> {
        <Self as AiPlayerTrait>::update(self)
    }

    /// Main AI think loop with frame parameter.
    ///
    /// C++ `AIPlayer::update` order (AIPlayer.cpp):
    ///   doBaseBuilding → checkReadyTeams → checkQueuedTeams →
    ///   doTeamBuilding → doUpgradesAndSkills → updateBridgeRepair
    ///
    /// Optional host residuals (off by default for C++ parity):
    /// - analysis prep before phases (`GENERALS_AI_HOST_ANALYSIS=1`)
    /// - strength-threshold attack after phases (`GENERALS_AI_HOST_ATTACK=1`)
    /// C++ skirmish attacks come from team scripts / AIGroup, not AIPlayer::update.
    pub fn update_with_frame(&mut self, frame: u32) -> Result<(), AiError> {
        if Self::host_analysis_enabled() {
            // Host residual (not in C++ AIPlayer::update).
            self.analyze_economic_situation()?;
            self.analyze_military_situation()?;
            self.analyze_threats()?;
        }

        // --- C++ AIPlayer::update phase order (timers live inside do_* ) ---
        self.do_base_building()?;
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;
        // --- end C++ phase order ---

        // Host residual: strength-threshold attack (not in C++ AIPlayer::update).
        // Default off — opt in with GENERALS_AI_HOST_ATTACK=1 for host smoke gates.
        if Self::host_attack_enabled() {
            self.process_attack_decisions(frame)?;
        }

        Ok(())
    }

    /// Opt-in host residual attack after C++ update phases.
    pub(super) fn host_attack_enabled() -> bool {
        match std::env::var("GENERALS_AI_HOST_ATTACK") {
            Ok(v) => {
                let v = v.trim();
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("on")
                    || v.eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        }
    }

    /// Opt-in host residual analysis before C++ update phases.
    pub(super) fn host_analysis_enabled() -> bool {
        match std::env::var("GENERALS_AI_HOST_ANALYSIS") {
            Ok(v) => {
                let v = v.trim();
                v == "1"
                    || v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("on")
                    || v.eq_ignore_ascii_case("yes")
            }
            Err(_) => false,
        }
    }

    pub(super) fn process_attack_decisions(&mut self, _frame: u32) -> Result<(), AiError> {
        let strength = self.military_state.total_military_strength;
        let threat = self.threat_assessment.overall_threat_level;

        if strength <= 0.0 {
            return Ok(());
        }

        let attack_ratio = match self.difficulty {
            GameDifficulty::Easy => 1.5,
            GameDifficulty::Normal => 1.0,
            GameDifficulty::Hard => 0.7,
            GameDifficulty::Brutal => 0.5,
        };

        if strength >= threat * attack_ratio {
            self.launch_attack()?;
        }

        Ok(())
    }

    pub(super) fn analyze_military_situation(&mut self) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        let mut total_strength = 0.0f32;
        let mut counts: HashMap<String, i32> = HashMap::new();

        for obj_id in player_guard.get_all_objects() {
            let Some(kind) = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if obj_guard.is_destroyed() || obj_guard.is_effectively_dead() {
                        return None;
                    }
                    if obj_guard.is_kind_of(KindOf::Infantry) {
                        Some("infantry")
                    } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                        Some("vehicle")
                    } else if obj_guard.is_kind_of(KindOf::Aircraft) {
                        Some("aircraft")
                    } else {
                        None
                    }
                })
                .flatten()
            else {
                continue;
            };
            match kind {
                "infantry" => {
                    *counts.entry("infantry".to_string()).or_insert(0) += 1;
                    total_strength += 1.0;
                }
                "vehicle" => {
                    *counts.entry("vehicle".to_string()).or_insert(0) += 1;
                    total_strength += 2.0;
                }
                "aircraft" => {
                    *counts.entry("aircraft".to_string()).or_insert(0) += 1;
                    total_strength += 3.0;
                }
                _ => {}
            }
        }

        self.military_state.unit_counts_by_type = counts;
        self.military_state.total_military_strength = total_strength;

        Ok(())
    }

    pub(super) fn analyze_threats(&mut self) -> Result<(), AiError> {
        // Wave 255: empty dual-world → no-op success.

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(player_arc) = self.get_player_arc() else {
            return Ok(());
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(());
        };

        let base_center = self.get_base_center().unwrap_or_else(Coord3D::origin);
        let scan_radius = 500.0f32;

        let mut threat_level = 0.0f32;
        let mut immediate_threats = Vec::new();

        if let Some(partition) = ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&base_center, scan_radius) {
                let Some((owner_id, target_team_arc, severity, location)) = OBJECT_REGISTRY
                    .with_object(obj_id, |obj_guard| {
                        if obj_guard.is_destroyed() {
                            return None;
                        }
                        let owner_id = obj_guard.get_controlling_player_id()?;
                        let target_team_arc = obj_guard.get_team()?;
                        let severity = if obj_guard.is_kind_of(KindOf::Structure) {
                            0.3
                        } else if obj_guard.is_kind_of(KindOf::Vehicle) {
                            0.7
                        } else if obj_guard.is_kind_of(KindOf::Infantry) {
                            0.5
                        } else if obj_guard.is_kind_of(KindOf::Aircraft) {
                            0.8
                        } else {
                            0.2
                        };
                        Some((
                            owner_id,
                            target_team_arc,
                            severity,
                            *obj_guard.get_position(),
                        ))
                    })
                    .flatten()
                else {
                    continue;
                };
                if owner_id as u32 == self.player_id {
                    continue;
                }
                if let Some(owner_arc) = player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(owner_id as i32).cloned())
                {
                    if let Ok(owner_guard) = owner_arc.read() {
                        if owner_guard.get_player_type() == PlayerType::Neutral {
                            continue;
                        }
                    }
                }
                let Ok(target_team) = target_team_arc.read() else {
                    continue;
                };
                if player_guard.get_relationship_with_team(&target_team) != Relationship::Enemies {
                    continue;
                }

                threat_level += severity;

                immediate_threats.push(ThreatInfo {
                    threat_id: obj_id,
                    threat_type: ThreatType::Military,
                    location,
                    severity,
                    time_detected: TheGameLogic::get_frame(),
                    estimated_time_to_impact: 0,
                });
            }
        }

        self.threat_assessment.immediate_threats = immediate_threats;
        self.threat_assessment.overall_threat_level = threat_level;

        self.threat_assessment.recommended_response = if threat_level > 5.0 {
            ThreatResponse::Emergency
        } else if threat_level > 3.0 {
            ThreatResponse::Attack
        } else if threat_level > 1.0 {
            ThreatResponse::Defend
        } else if threat_level > 0.0 {
            ThreatResponse::Monitor
        } else {
            ThreatResponse::None
        };

        self.military_state.enemy_strength_estimate = threat_level * 2.0;

        Ok(())
    }

    pub fn get_build_delay(&self) -> u32 {
        self.build_delay
    }

    pub fn get_team_delay(&self) -> u32 {
        self.team_delay
    }

    pub fn get_team_timer(&self) -> u32 {
        self.team_timer
    }

    pub fn get_structure_timer(&self) -> u32 {
        self.structure_timer
    }

    pub fn set_build_delay_frames(&mut self, frames: u32) {
        self.build_delay = frames;
    }

    pub fn set_team_delay_frames(&mut self, frames: u32) {
        self.team_delay = frames;
    }

    pub fn set_team_timer_frames(&mut self, frames: u32) {
        self.team_timer = frames;
    }

    pub fn set_structure_timer_frames(&mut self, frames: u32) {
        self.structure_timer = frames;
    }

    pub fn can_build_structure_now(&self) -> bool {
        self.ready_to_build_structure && self.build_delay == 0
    }

    pub fn can_build_team_now(&self) -> bool {
        self.ready_to_build_team && self.team_delay == 0
    }

    pub fn is_ready_to_build_structure(&self) -> bool {
        self.ready_to_build_structure
    }

    pub fn set_ready_to_build_structure(&mut self, ready: bool) {
        self.ready_to_build_structure = ready;
    }

    pub fn is_ready_to_build_team(&self) -> bool {
        self.ready_to_build_team
    }

    pub fn set_ready_to_build_team(&mut self, ready: bool) {
        self.ready_to_build_team = ready;
    }

    pub fn start_structure_timer_seconds(&mut self, seconds: i32) {
        let seconds = seconds.max(0) as u32;
        self.structure_timer = seconds * LOGICFRAMES_PER_SECOND;
        self.ready_to_build_structure = false;
    }

    /// Returns true if the team is already queued for building.
    pub fn is_team_in_queue(&self, team_name: &str) -> bool {
        self.team_build_queue.iter().any(|team| {
            team.team_name
                .as_deref()
                .map(|name| name == team_name)
                .unwrap_or(false)
        })
    }

    /// Check if location is safe for building.
    /// C++ `AIPlayer::isLocationSafe` (AIPlayer.cpp).
    ///
    /// Scan enemies (alive, non-stealthed, significant, non-harvester, non-dozer)
    /// within supply-center safe radius + template bounding radius.
    /// C++ `AIPlayer::isLocationSafe` (AIPlayer.cpp).
    ///
    /// Partition closest-object filters: enemies only, alive, not stealthed,
    /// reject harvesters/dozers. Any hit → unsafe.
    pub fn is_location_safe(&self, pos: &Coord3D, thing: &dyn ThingTemplate) -> bool {
        // Wave 255: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(player_arc) = self.get_player_arc() else {
            return true;
        };
        let Ok(player_guard) = player_arc.read() else {
            return true;
        };
        let Some(partition) = ThePartitionManager::get() else {
            return true;
        };

        let aidata_r = THE_AI.read().ok().and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|d| d.supply_center_safe_radius)
        });
        let radius = leftover_is_location_safe_radius(
            aidata_r,
            thing
                .get_template_geometry_info()
                .get_bounding_circle_radius(),
        );

        let mut candidates: Vec<LeftoverLocationSafeCandidate> = Vec::new();
        for obj_id in partition.get_objects_in_range(pos, radius) {
            if let Some(c) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                // PartitionFilterAlive / harvester / dozer / stealth / enemies
                let is_enemy = obj_guard
                    .get_team()
                    .and_then(|team_arc| {
                        team_arc.read().ok().map(|team| {
                            player_guard.get_relationship_with_team(&team) == Relationship::Enemies
                        })
                    })
                    .unwrap_or(false);
                let p = obj_guard.get_position();
                LeftoverLocationSafeCandidate {
                    x: p.x,
                    y: p.y,
                    is_destroyed: obj_guard.is_destroyed(),
                    is_effectively_dead: obj_guard.is_effectively_dead(),
                    is_harvester: obj_guard.is_kind_of(KindOf::Harvester),
                    is_dozer: obj_guard.is_kind_of(KindOf::Dozer),
                    stealthed: obj_guard.test_status(ObjectStatusTypes::Stealthed),
                    detected: obj_guard.test_status(ObjectStatusTypes::Detected),
                    disguised: obj_guard.test_status(ObjectStatusTypes::Disguised),
                    is_enemy,
                    is_bridge: obj_guard.is_kind_of(KindOf::Bridge),
                    is_bridge_tower: obj_guard.is_kind_of(KindOf::BridgeTower),
                }
            }) {
                candidates.push(c);
            }
        }
        leftover_is_location_safe(pos.x, pos.y, radius, candidates)
    }

    /// Update loop variant for skirmish AI that supplies its own base-building logic.
    ///
    /// C++ order without `doBaseBuilding` (skirmish overrides base building).
    pub fn update_without_base_building(&mut self) -> Result<(), AiError> {
        self.check_ready_teams()?;
        self.check_queued_teams()?;
        self.do_team_building()?;
        self.do_upgrades_and_skills()?;
        self.update_bridge_repair()?;
        Ok(())
    }

    /// C++ `AIPlayer::newMap` (AIPlayer.cpp).
    ///
    /// 1. Snapshot pre-existing build-list entries (C++ saves head before prepends)
    /// 2. Prepend placed factories via addToBuildList
    /// 3. computeCenterAndRadiusOfBase (includes new factories)
    /// 4. Walk *original* entries only: initiallyBuilt → buildStructureNow else
    ///    incrementNumRebuilds
    pub fn new_map(&mut self) {
        // Wave 255: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        // C++ does not clear queues/timers here — only factory scan + initial builds.

        // Snapshot original build list BEFORE factory prepends (C++ keeps old head ptr).
        let mut original_entries: Vec<(String, Coord3D, Real, bool)> = Vec::new();
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                if let Ok(pg) = player_arc.read() {
                    let mut cur = pg.get_build_list();
                    while let Some(node) = cur {
                        let name = node.get_template_name().to_string();
                        if !name.is_empty() {
                            original_entries.push((
                                name,
                                *node.get_location(),
                                node.get_angle(),
                                node.is_initially_built(),
                            ));
                        }
                        cur = node.get_next();
                    }
                }
            }
        }

        // Add any factories placed to the build list (C++ ProductionUpdateInterface).
        // C++ addToBuildList prepends — new entries are NOT in original_entries.
        if let Ok(list) = player_list().read() {
            if let Some(player_arc) = list.get_player(self.player_id as i32) {
                let owned: Vec<ObjectID> = player_arc
                    .read()
                    .ok()
                    .map(|g| g.get_all_objects())
                    .unwrap_or_default();
                drop(list);
                for obj_id in owned {
                    let Some((template_name, pos, angle)) = OBJECT_REGISTRY
                        .with_object(obj_id, |obj_g| {
                            // Factory if any production interface.
                            let mut is_factory = false;
                            for behavior in obj_g.get_behavior_modules() {
                                if let Ok(mut bg) = behavior.lock() {
                                    if bg.get_production_update_interface().is_some() {
                                        is_factory = true;
                                        break;
                                    }
                                }
                            }
                            if !is_factory {
                                for mh in obj_g.behavior_modules() {
                                    let matched = mh.with_module(|module| {
                                        module.get_production_control_interface().is_some()
                                    });
                                    if matched {
                                        is_factory = true;
                                        break;
                                    }
                                }
                            }
                            if !is_factory {
                                return None;
                            }
                            Some((
                                obj_g.get_template_name().to_string(),
                                *obj_g.get_position(),
                                obj_g.get_orientation(),
                            ))
                        })
                        .flatten()
                    else {
                        continue;
                    };
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(self.player_id as i32) {
                            if let Ok(mut pg) = player_arc.write() {
                                pg.add_to_build_list(
                                    obj_id,
                                    AsciiString::from(template_name.as_str()),
                                    pos,
                                    angle,
                                );
                            }
                        }
                    }
                }
            }
        }

        let _ = self.compute_center_and_radius_of_base();

        // Walk original (pre-factory) entries only — matches C++ head pointer walk.
        let mut initial: Vec<(String, Coord3D, Real)> = Vec::new();
        for (name, loc, ang, initially) in original_entries {
            if TheThingFactory::find_template(&name).is_none() {
                log::debug!("*** ERROR - Build list building '{}' doesn't exist.", name);
                continue;
            }
            if initially {
                initial.push((name, loc, ang));
            } else {
                // C++ info->incrementNumRebuilds on the live node.
                if let Ok(list) = player_list().read() {
                    if let Some(player_arc) = list.get_player(self.player_id as i32) {
                        if let Ok(mut pg) = player_arc.write() {
                            if let Some(info) = pg.get_build_list_mut() {
                                let mut cur = Some(&mut *info);
                                while let Some(node) = cur {
                                    if node.get_template_name() == name
                                        && (node.get_location().x - loc.x).abs() < 0.01
                                        && (node.get_location().y - loc.y).abs() < 0.01
                                    {
                                        node.increment_num_rebuilds();
                                        break;
                                    }
                                    cur = node.get_next_mut();
                                }
                            }
                        }
                    }
                }
            }
        }
        for (name, loc, ang) in initial {
            if let Err(err) = self.build_structure_now_at(&name, loc, ang, None) {
                log::debug!("newMap buildStructureNow('{}') failed: {err}", name);
            }
        }
    }

    /// Start training for a work order with factory management.
    pub(crate) fn start_training_for_order(
        &mut self,
        order: &mut WorkOrder,
        busy_ok: bool,
    ) -> Result<bool, AiError> {
        self.start_training_internal(order, busy_ok, "default")
    }
}
