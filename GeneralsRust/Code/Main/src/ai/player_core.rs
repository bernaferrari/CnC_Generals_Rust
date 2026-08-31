use super::*;

impl AIPlayer {
    /// Create new AI player
    pub fn new(player_id: u32, team: Team, difficulty: AIDifficulty) -> Self {
        let personality = AIPersonality::for_team(team);
        // C++ AIPlayer.cpp:71 p->setCanBuildUnits(false) on leftover PlayerList.
        if let Ok(list) = gamelogic::player::player_list().read() {
            if let Some(player_arc) = list.get_player(player_id as i32).cloned() {
                if let Ok(mut player) = player_arc.write() {
                    player.set_can_build_units(false);
                }
            }
        }
        Self {
            player_id,
            team,
            difficulty,
            personality,
            is_active: true,
            enemy_player_id: None,
            base_center: Vec3::ZERO,
            base_radius: 100.0,
            // Seed from player id (stable per slot); base_center updates don't reseed.
            placement_rng: HostRandomState::seeded(player_id.wrapping_add(0xA17A_0001)),
            building_queue: Vec::new(),
            next_building_time: 0.0,
            next_team_queue_time: 0.0,
            next_team_time: 0.0,
            team_seconds: Self::TEAM_SECONDS,
            team_queue: VecDeque::new(),
            team_ready_queue: VecDeque::new(),
            attack_in_progress: false,
            last_attack_time: 0.0,
            defensive_units: Vec::new(),
            last_update_time: 0.0,
            resource_check_time: 0.0,
            enemy_check_time: 0.0,
            current_strategy: AIStrategy::EarlyGame,
            build_phase: AIBuildPhase::BaseConstruction,
            activity_count: 0,
            skillset_selector: INVALID_SKILLSET_SELECTION,
            peer_ai_targets: Vec::new(),
            structures_to_repair: Vec::new(),
            repair_dozer: None,
            repair_dozer_origin: Vec3::ZERO,
            dozer_queued_for_repair: false,
            dozer_is_repairing: false,
            last_bridge_repair_time: -1.0,
            cur_front_base_defense: 0,
            cur_flank_base_defense: 0,
            cur_front_left_defense_angle: 0.0,
            cur_front_right_defense_angle: 0.0,
            cur_left_flank_left_defense_angle: 0.0,
            cur_left_flank_right_defense_angle: 0.0,
            cur_right_flank_left_defense_angle: 0.0,
            cur_right_flank_right_defense_angle: 0.0,
            skirmish_new_map_applied: false,
            current_warehouse_id: None,
            supply_source_attack_check_frame: 0,
            attacked_supply_center: None,
        }
    }

    /// Initialize AI with starting base layout
    pub fn initialize(&mut self, base_position: Vec3) {
        self.base_center = base_position;
        self.setup_base_layout();
        self.setup_initial_strategy();
        // Act on the first host AI update (skirmish vertical-slice pacing).
        self.next_building_time = 0.0;
        self.next_team_queue_time = 0.0;
        self.next_team_time = 0.0;
        self.enemy_check_time = 0.0;
        // C++-aligned: no artificial negative last_attack to force immediate attacks.
        self.last_attack_time = 0.0;
    }

    /// C++ `AIPlayer::setTeamDelaySeconds`.
    pub fn set_team_delay_seconds(&mut self, delay: i32) {
        self.team_seconds = delay.max(0) as f32;
    }

    pub fn capture_queue_persist(
        &self,
    ) -> crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist {
        use crate::save_load::snapshot::ai_player_queue_persist::{
            AIPlayerQueuePersist, AITeamQueuePersist,
        };
        AIPlayerQueuePersist {
            player_id: self.player_id,
            team_queue: self
                .team_queue
                .iter()
                .map(AITeamQueuePersist::from_live)
                .collect(),
            team_ready_queue: self
                .team_ready_queue
                .iter()
                .map(AITeamQueuePersist::from_live)
                .collect(),
            next_building_time: self.next_building_time,
            next_team_queue_time: self.next_team_queue_time,
            next_team_time: self.next_team_time,
            team_seconds: self.team_seconds,
            last_update_time: self.last_update_time,
            current_warehouse_id: self.current_warehouse_id.map(|id| id.0),
            repair_dozer: self.repair_dozer.map(|id| id.0),
            repair_dozer_origin: [
                self.repair_dozer_origin.x,
                self.repair_dozer_origin.y,
                self.repair_dozer_origin.z,
            ],
            structures_to_repair: self.structures_to_repair.iter().map(|id| id.0).collect(),
            dozer_queued_for_repair: self.dozer_queued_for_repair,
            dozer_is_repairing: self.dozer_is_repairing,
            last_bridge_repair_time: self.last_bridge_repair_time,
            skillset_selector: self.skillset_selector,
            cur_front_base_defense: self.cur_front_base_defense,
            cur_flank_base_defense: self.cur_flank_base_defense,
            cur_front_left_defense_angle: self.cur_front_left_defense_angle,
            cur_front_right_defense_angle: self.cur_front_right_defense_angle,
            cur_left_flank_left_defense_angle: self.cur_left_flank_left_defense_angle,
            cur_left_flank_right_defense_angle: self.cur_left_flank_right_defense_angle,
            cur_right_flank_left_defense_angle: self.cur_right_flank_left_defense_angle,
            cur_right_flank_right_defense_angle: self.cur_right_flank_right_defense_angle,
            building_rebuild_counts: self
                .building_queue
                .iter()
                .map(|building| building.rebuild_count)
                .collect(),
            building_destroyed_at_times: self
                .building_queue
                .iter()
                .map(|building| building.destroyed_at_time)
                .collect(),
            building_object_ids: self
                .building_queue
                .iter()
                .map(|building| building.object_id.map(|id| id.0))
                .collect(),
            building_is_built: self
                .building_queue
                .iter()
                .map(|building| building.is_built)
                .collect(),
            building_is_priority: self
                .building_queue
                .iter()
                .map(|building| building.is_priority)
                .collect(),
        }
    }

    pub fn apply_queue_persist(
        &mut self,
        persist: crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist,
    ) {
        use crate::save_load::snapshot::ai_player_queue_persist::AITeamQueuePersist;
        self.team_queue = persist
            .team_queue
            .into_iter()
            .map(AITeamQueuePersist::into_live)
            .collect();
        self.team_ready_queue = persist
            .team_ready_queue
            .into_iter()
            .map(AITeamQueuePersist::into_live)
            .collect();
        self.next_building_time = persist.next_building_time;
        self.next_team_queue_time = persist.next_team_queue_time;
        self.next_team_time = persist.next_team_time;
        self.team_seconds = persist.team_seconds;
        self.last_update_time = persist.last_update_time;
        self.current_warehouse_id = persist.current_warehouse_id.map(ObjectId);
        self.repair_dozer = persist.repair_dozer.map(ObjectId);
        self.repair_dozer_origin = glam::Vec3::new(
            persist.repair_dozer_origin[0],
            persist.repair_dozer_origin[1],
            persist.repair_dozer_origin[2],
        );
        self.structures_to_repair = persist
            .structures_to_repair
            .into_iter()
            .map(ObjectId)
            .collect();
        self.dozer_queued_for_repair = persist.dozer_queued_for_repair;
        self.dozer_is_repairing = persist.dozer_is_repairing;
        self.last_bridge_repair_time = persist.last_bridge_repair_time;
        self.skillset_selector = persist.skillset_selector;
        self.cur_front_base_defense = persist.cur_front_base_defense;
        self.cur_flank_base_defense = persist.cur_flank_base_defense;
        self.cur_front_left_defense_angle = persist.cur_front_left_defense_angle;
        self.cur_front_right_defense_angle = persist.cur_front_right_defense_angle;
        self.cur_left_flank_left_defense_angle = persist.cur_left_flank_left_defense_angle;
        self.cur_left_flank_right_defense_angle = persist.cur_left_flank_right_defense_angle;
        self.cur_right_flank_left_defense_angle = persist.cur_right_flank_left_defense_angle;
        self.cur_right_flank_right_defense_angle = persist.cur_right_flank_right_defense_angle;
        // Leftover Player::xfer remaining num_rebuilds. Write spent count only;
        // do not rebuild the INI layout or clear object/under-construction fields.
        for (building, remaining) in self
            .building_queue
            .iter_mut()
            .zip(persist.building_rebuild_counts)
        {
            building.rebuild_count = remaining;
        }
        // Leftover Player::xfer object_timestamp rebuild-delay clock.
        for (building, stamp) in self
            .building_queue
            .iter_mut()
            .zip(persist.building_destroyed_at_times)
        {
            building.destroyed_at_time = stamp;
        }
        // Leftover Player::xfer object_id / under_construction / priority_build.
        for (building, object_id) in self
            .building_queue
            .iter_mut()
            .zip(persist.building_object_ids)
        {
            building.object_id = object_id.map(ObjectId);
        }
        for (building, is_built) in self
            .building_queue
            .iter_mut()
            .zip(persist.building_is_built)
        {
            building.is_built = is_built;
        }
        for (building, is_priority) in self
            .building_queue
            .iter_mut()
            .zip(persist.building_is_priority)
        {
            building.is_priority = is_priority;
        }
    }

    pub fn clear_queue_persist(&mut self) {
        self.team_queue.clear();
        self.team_ready_queue.clear();
        self.next_building_time = 0.0;
        self.next_team_queue_time = 0.0;
        self.next_team_time = 0.0;
        self.current_warehouse_id = None;
        self.repair_dozer = None;
        self.structures_to_repair.clear();
        self.dozer_queued_for_repair = false;
        self.dozer_is_repairing = false;
        self.last_bridge_repair_time = -1.0;
        self.skillset_selector = INVALID_SKILLSET_SELECTION;
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.cur_front_left_defense_angle = 0.0;
        self.cur_front_right_defense_angle = 0.0;
        self.cur_left_flank_left_defense_angle = 0.0;
        self.cur_left_flank_right_defense_angle = 0.0;
        self.cur_right_flank_left_defense_angle = 0.0;
        self.cur_right_flank_right_defense_angle = 0.0;
    }

    pub fn retain_queue_object_ids(&mut self, valid: &std::collections::HashSet<ObjectId>) {
        self.structures_to_repair.retain(|id| valid.contains(id));
        if self.repair_dozer.is_some_and(|id| !valid.contains(&id)) {
            self.repair_dozer = None;
        }
        if self
            .current_warehouse_id
            .is_some_and(|id| !valid.contains(&id))
        {
            self.current_warehouse_id = None;
        }
        for team in self
            .team_queue
            .iter_mut()
            .chain(self.team_ready_queue.iter_mut())
        {
            if team.reinforcement_id.is_some_and(|id| !valid.contains(&id)) {
                team.reinforcement_id = None;
            }
            for order in &mut team.work_orders {
                if order.factory_id.is_some_and(|id| !valid.contains(&id)) {
                    order.factory_id = None;
                }
                if order
                    .supply_center_id
                    .is_some_and(|id| !valid.contains(&id))
                {
                    order.supply_center_id = None;
                }
                order.observed_unit_ids.retain(|id| valid.contains(id));
            }
        }
        for building in &mut self.building_queue {
            if building.object_id.is_some_and(|id| !valid.contains(&id)) {
                building.object_id = None;
                building.is_built = false;
            }
        }
    }

    /// Relocate base center and re-seed the structure build queue at the new site.
    ///
    /// Used by host golden combat so AI rebuild soup stays within production-weapon
    /// range without stripping faction templates from the catalog.
    pub fn relocate_base(&mut self, base_position: Vec3) {
        self.base_center = base_position;
        self.building_queue.clear();
        self.setup_base_layout();
    }

    /// Main AI update — C++ `AIPlayer::update` (`AIPlayer.cpp:2987-3002`):
    /// doBaseBuilding → checkReadyTeams → checkQueuedTeams → doTeamBuilding
    /// → doUpgradesAndSkills → updateBridgeRepair.
    /// `AISkirmishPlayer::update` just calls this (`AISkirmishPlayer.cpp:932-935`).
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if !self.is_active {
            return;
        }

        // C++ AISkirmishPlayer::newMap — AIData SideBuildList replaces invented pads.
        self.ensure_skirmish_new_map(game_logic);

        self.last_update_time = current_time;
        self.update_enemy_assessment(game_logic, current_time);
        // C++ Player::preTeamDestroy / AIPlayer::aiPreTeamDestroy — drop wiped
        // TeamInQueue entries before checkReadyTeams / checkQueuedTeams.
        self.purge_destroyed_or_wiped_queued_teams(game_logic);

        // doBaseBuilding
        self.update_economic_management(game_logic, current_time);
        // checkReadyTeams — activate ready-queue teams (AIPlayer.cpp:2729-2803)
        self.check_ready_teams(game_logic, current_time);
        // checkQueuedTeams — expire / disband / promote (AIPlayer.cpp:2810-2870)
        self.check_queued_teams(game_logic, current_time);
        // doTeamBuilding
        self.update_military_management(game_logic, current_time);
        self.do_upgrades_and_skills(game_logic);
        // updateBridgeRepair — C++ AIPlayer::update never calls checkBridges
        // (scripts do, via leftover findBrokenBridge + clientSafeQuickDoesPathExist).
        self.update_bridge_repair(game_logic, current_time);

        self.update_strategic_decisions(game_logic, current_time);
        // C++ AIPlayer::update never auto-fires specials. Named
        // SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST goes through
        // fire_named_special_power after leftover script dispatch.
    }

    /// Set up initial base building layout.
    ///
    /// Core pads plus the C++ `AIData.ini` `SkirmishBuildList` tech/air
    /// structures and `SideInfo::BaseDefenseStructure1`.  Core pads stay
    /// inside the host 512² MinDistFromEdge residual (|offset| ≤ 100).
    /// Defenses use the C++ approach-path fan, not a fixed +80/+80 pad.
    pub(super) fn setup_base_layout(&mut self) {
        let center = self.base_center;
        self.reset_base_defense_fan();

        // Core base buildings based on team. C++ `newMap` increments rebuilds
        // so the first construction does not spend the last rebuild, and
        // `UNLIMITED_REBUILDS` is a no-op decrement.
        match self.team {
            Team::USA => {
                self.add_layout_building("AmericaCommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "AmericaSupplyCenter",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaPowerPlant",
                    center + Vec3::new(-50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaBarracks",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaWarFactory",
                    center + Vec3::new(100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList America: StrategyCenter + Airfield.
                self.add_layout_building(
                    "AmericaStrategyCenter",
                    center + Vec3::new(-100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "AmericaAirfield",
                    center + Vec3::new(50.0, 0.0, -100.0),
                    UNLIMITED_REBUILDS,
                );
            }
            Team::China => {
                self.add_layout_building("ChinaCommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "ChinaSupplyCenter",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaPowerPlant",
                    center + Vec3::new(-50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaBarracks",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaWarFactory",
                    center + Vec3::new(100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList China: PropagandaCenter + Airfield.
                self.add_layout_building(
                    "ChinaPropagandaCenter",
                    center + Vec3::new(-100.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "ChinaAirfield",
                    center + Vec3::new(50.0, 0.0, -100.0),
                    UNLIMITED_REBUILDS,
                );
            }
            Team::GLA => {
                self.add_layout_building("GLACommandCenter", center, UNLIMITED_REBUILDS);
                self.add_layout_building(
                    "GLASupplyStash",
                    center + Vec3::new(50.0, 0.0, 0.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "GLAArmsDealer",
                    center + Vec3::new(0.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                self.add_layout_building(
                    "GLABarracks",
                    center + Vec3::new(-50.0, 0.0, 50.0),
                    UNLIMITED_REBUILDS,
                );
                // C++ SkirmishBuildList GLA: Palace is the tech structure.
                self.add_layout_building(
                    "GLAPalace",
                    center + Vec3::new(100.0, 0.0, -50.0),
                    UNLIMITED_REBUILDS,
                );
            }
            _ => {}
        }

        // C++ `AISkirmishPlayer::buildAIBaseDefense` — first front-fan slot.
        self.queue_front_base_defense(None);
    }

    /// C++ `AISkirmishPlayer::newMap` — replace invented pads with AIData list.
    pub(super) fn ensure_skirmish_new_map(&mut self, game_logic: &mut GameLogic) {
        if self.skirmish_new_map_applied {
            return;
        }
        self.skirmish_new_map_applied = true;
        if game_logic
            .get_player(self.player_id)
            .is_some_and(|p| !p.map_side.build_list.is_empty())
        {
            // Campaign/map SidesList already consumed by feed_host_ai.
            return;
        }
        let _ = self.apply_skirmish_new_map(game_logic);
    }

    /// C++ `AISkirmishPlayer::newMap` + `adjustBuildList`.
    pub fn apply_skirmish_new_map(&mut self, game_logic: &mut GameLogic) -> bool {
        let Some(side) = self.side_info_name() else {
            return false;
        };
        let Some(entries) = Self::aidata_side_build_entries(side) else {
            return false;
        };
        if entries.is_empty() {
            return false;
        }

        let start_pos = self.destroy_owned_command_center(game_logic);
        let Some(start_pos) = start_pos.or(Some(self.base_center)) else {
            return false;
        };

        let mut list_cc: Option<Vec3> = None;
        let mut marked: Vec<SideBuildPad> = Vec::new();
        for mut entry in entries {
            if Self::template_is_command_center(game_logic, &entry.template) {
                entry.initially_built = true;
                if list_cc.is_none() {
                    list_cc = Some(entry.position);
                }
            }
            marked.push(entry);
        }
        let Some(build_pos) = list_cc else {
            return false;
        };

        let rotate = Self::aidata_rotate_skirmish_bases();
        let (lo, hi) = game_logic.world_bounds();
        let width = (hi.x - lo.x).max(1.0);
        let height = (hi.z - lo.z).max(1.0);
        let mut grid_index = 0;
        if start_pos.x > lo.x + width / 3.0 {
            grid_index += 1;
        }
        if start_pos.x > lo.x + 2.0 * width / 3.0 {
            grid_index += 1;
        }
        if start_pos.z > lo.z + height / 3.0 {
            grid_index += 3;
        }
        if start_pos.z > lo.z + 2.0 * height / 3.0 {
            grid_index += 3;
        }
        let mut angle = if rotate {
            match grid_index {
                0 => 0.0,
                1 => std::f32::consts::PI / 4.0,
                2 => std::f32::consts::PI / 2.0,
                3 => -std::f32::consts::PI / 4.0,
                4 => 0.0,
                5 => 3.0 * std::f32::consts::PI / 4.0,
                6 => -std::f32::consts::PI / 2.0,
                7 => -3.0 * std::f32::consts::PI / 4.0,
                _ => std::f32::consts::PI,
            }
        } else {
            0.0
        };
        angle += 3.0 * std::f32::consts::PI / 4.0;
        let s = angle.sin();
        let c = angle.cos();

        self.building_queue.clear();
        self.reset_base_defense_fan();
        for entry in marked {
            let mut pos = entry.position;
            pos.x -= build_pos.x;
            pos.z -= build_pos.z;
            let new_x = pos.x * c - pos.z * s;
            let new_z = pos.z * c + pos.x * s;
            pos.x = new_x + start_pos.x;
            pos.z = new_z + start_pos.z;
            pos.y = 0.0;

            let rebuilds = if entry.rebuilds < 0 {
                UNLIMITED_REBUILDS
            } else {
                entry.rebuilds as u32
            };
            let mut building = AIBuildingInfo::new(entry.template.clone(), pos, rebuilds);
            building.automatic_build = entry.automatically_build;
            building.is_priority = false;
            if entry.initially_built {
                // C++ buildStructureNow — do not incrementNumRebuilds on CC.
                building.is_built = true;
                if let Some(id) = game_logic.create_object(&entry.template, self.team, pos) {
                    if let Some(obj) = game_logic.host_object_mut(id) {
                        obj.owner_player_id = Some(self.player_id);
                    }
                    building.object_id = Some(id);
                }
            } else {
                building.increment_num_rebuilds();
            }
            self.building_queue.push(building);
        }
        self.compute_center_and_radius_of_base(game_logic);
        true
    }

    /// C++ `AIPlayer::computeCenterAndRadiusOfBase`.
    /// Leftover-calls leftover centroid + axis-abs + geom*0.4 hypot.
    pub fn compute_center_and_radius_of_base(&mut self, game_logic: &GameLogic) {
        let mut entries: Vec<(f32, f32, f32)> = Vec::new();
        for building in &self.building_queue {
            if building.template_name.is_empty() {
                continue;
            }
            let Some(template) = game_logic.templates.get(&building.template_name) else {
                continue;
            };
            // C++ `getTemplateGeometryInfo().getBoundingCircleRadius()`:
            // always the template geometry, never a zero fallback.
            // Cylinder/sphere bounding circle == majorRadius (Geometry.cpp
            // calcBoundingStuff), so major_radius is the C++ metric.
            let geom_r = template.geometry_info.bounding_circle_radius();
            entries.push((building.position.x, building.position.z, geom_r));
        }
        let (set, cx, cz, radius) =
            gamelogic::ai::ai_player::leftover_compute_center_and_radius_of_base(&entries);
        if set {
            self.base_center = Vec3::new(cx, 0.0, cz);
            self.base_radius = radius;
        } else {
            self.base_center = Vec3::ZERO;
            self.base_radius = 0.0;
        }
    }

    pub(super) fn destroy_owned_command_center(
        &mut self,
        game_logic: &mut GameLogic,
    ) -> Option<Vec3> {
        let mut found = None;
        for (&id, object) in game_logic.host_objects() {
            if !object.is_alive() {
                continue;
            }
            let ours = object.owner_player_id == Some(self.player_id)
                || (object.owner_player_id.is_none() && object.team == self.team);
            if !ours || !object.is_kind_of(KindOf::CommandCenter) {
                continue;
            }
            found = Some((id, object.get_position()));
            break;
        }
        let (id, pos) = found?;
        game_logic.destroy_object(id);
        // C++ newMap destroys the map-placed CC synchronously (destroyObject +
        // ThePartitionManager->removeObject before applying the build list).
        // Our StructureTopple residual defers removal to a death animation that
        // is normally driven by world tick; drive it to completion here and
        // flush the destroy queue so the CC leaves the host object store in
        // this call, matching C++ newMap ordering.
        let frame = game_logic.get_frame();
        for step in 1..=2000u32 {
            if let Some(obj) = game_logic.host_object_mut(id) {
                if obj.tick_structure_topple(frame.saturating_add(step)) {
                    break;
                }
            } else {
                break;
            }
        }
        // Topple Done lets the next destroy pass enqueue (the deferral peel
        // swallowed the original DestructionEvent), so re-mark and flush.
        if game_logic.host_object(id).is_some() {
            game_logic.destroy_object(id);
        }
        game_logic.process_destroy_list();
        Some(pos)
    }

    pub(super) fn template_is_command_center(game_logic: &GameLogic, template_name: &str) -> bool {
        if let Some(template) = game_logic.templates.get(template_name) {
            return template.is_kind_of(KindOf::CommandCenter);
        }
        template_name.contains("CommandCenter")
    }

    pub(super) fn aidata_rotate_skirmish_bases() -> bool {
        let store = game_engine::common::ini::get_ai_data_store();
        if let Some(data) = store.get_active() {
            return data.rotate_skirmish_bases;
        }
        drop(store);
        gamelogic::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.rotate_skirmish_bases)
            })
            .unwrap_or(false)
    }

    pub(super) fn aidata_max_recruit_distance() -> f32 {
        let from_store = (|| {
            let store = game_engine::common::ini::get_ai_data_store();
            store.get_active().map(|d| d.max_recruit_distance)
        })();
        let dist = from_store
            .or_else(|| {
                gamelogic::ai::THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.max_recruit_distance))
            })
            .unwrap_or(0.0);
        if dist > 0.0 {
            dist
        } else {
            99_999.0
        }
    }

    pub(super) fn aidata_side_build_entries(side: &str) -> Option<Vec<SideBuildPad>> {
        {
            let store = game_engine::common::ini::get_ai_data_store();
            if let Some(data) = store.get_active() {
                if let Some(list) = data
                    .side_build_lists
                    .iter()
                    .find(|l| l.side.eq_ignore_ascii_case(side))
                {
                    if !list.entries.is_empty() {
                        return Some(
                            list.entries
                                .iter()
                                .map(|e| SideBuildPad {
                                    template: e.template_name.clone(),
                                    position: Vec3::new(e.location.0, 0.0, e.location.1),
                                    rebuilds: e.rebuilds,
                                    initially_built: e.initially_built,
                                    automatically_build: e.automatically_build,
                                })
                                .collect(),
                        );
                    }
                }
            }
        }
        let ai = gamelogic::ai::THE_AI.read().ok()?;
        let data_arc = ai.get_ai_data();
        let data = data_arc.read().ok()?;
        let entry = data
            .side_build_lists
            .iter()
            .find(|e| e.side.eq_ignore_ascii_case(side))?;
        let list = entry.build_list.as_ref()?;
        let mut out = Vec::new();
        let mut cur = Some(list.as_ref());
        while let Some(info) = cur {
            let name = info.get_template_name().to_string();
            if !name.is_empty() {
                let loc = info.get_location().clone();
                out.push(SideBuildPad {
                    template: name,
                    position: Vec3::new(loc.x, loc.z, loc.y),
                    rebuilds: info.get_num_rebuilds() as i32,
                    initially_built: info.is_initially_built(),
                    automatically_build: info.is_automatic_build(),
                });
            }
            cur = info.get_next();
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// C++ `AIData.ini` `SideInfo` name for the live host team.
    /// Base-team fallback when no PlayerTemplate Side is bound.
    pub(super) fn side_info_name(&self) -> Option<&'static str> {
        use crate::game_logic::host_faction_skirmish_residual::{
            SIDE_AMERICA, SIDE_CHINA, SIDE_GLA,
        };
        match self.team {
            Team::USA => Some(SIDE_AMERICA),
            Team::China => Some(SIDE_CHINA),
            Team::GLA => Some(SIDE_GLA),
            _ => None,
        }
    }

    /// C++ `Player::getSide()` — PlayerTemplate Side (`AmericaAirForceGeneral`),
    /// not the three-value host `Team` enum.
    pub(super) fn live_player_side(&self, game_logic: &GameLogic) -> Option<String> {
        use crate::game_logic::host_faction_skirmish_residual::find_player_template_residual;
        // Residual Side matches C++ PlayerTemplate.ini (`AmericaAirForceGeneral`).
        // Prefer the bound identity so a leftover store that only copied
        // BaseSide=America cannot collapse ZH generals back to Paladin.
        if let Some(identity) = game_logic.player_template_identity(self.player_id) {
            if let Some(residual) = find_player_template_residual(&identity.template_name) {
                return Some(residual.side.to_string());
            }
        }
        if let Some(template) = game_logic.resolved_player_template(self.player_id) {
            let side = template.get_side();
            if !side.is_empty() {
                return Some(side.to_string());
            }
        }
        self.side_info_name().map(str::to_string)
    }

    pub(super) fn science_names_from_skill_ids(num_skills: i32, skills: &[i32]) -> Vec<String> {
        use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
        let Some(store) = get_science_store() else {
            return Vec::new();
        };
        let n = (num_skills.max(0) as usize).min(skills.len());
        (0..n)
            .filter_map(|i| {
                let sci = skills[i];
                if sci == SCIENCE_INVALID {
                    return None;
                }
                let name = store.get_internal_name_for_science(sci);
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .collect()
    }

    /// Leftover parsed `AIData` `SideInfo` SkillSet1–5 (C++ AIPlayer.cpp:2919-2961).
    pub(super) fn aidata_side_skillsets(side: &str) -> Option<[Vec<String>; 5]> {
        {
            let store = game_engine::common::ini::get_ai_data_store();
            if let Some(data) = store.get_active() {
                if let Some(info) = data
                    .side_info
                    .iter()
                    .find(|info| info.side.eq_ignore_ascii_case(side))
                {
                    let sets = [
                        Self::science_names_from_skill_ids(
                            info.skill_set_1.num_skills,
                            &info.skill_set_1.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_2.num_skills,
                            &info.skill_set_2.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_3.num_skills,
                            &info.skill_set_3.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_4.num_skills,
                            &info.skill_set_4.skills,
                        ),
                        Self::science_names_from_skill_ids(
                            info.skill_set_5.num_skills,
                            &info.skill_set_5.skills,
                        ),
                    ];
                    if sets.iter().any(|set| !set.is_empty()) {
                        return Some(sets);
                    }
                }
            }
        }
        let ai = gamelogic::ai::THE_AI.read().ok()?;
        let ai_data = ai.get_ai_data();
        let data = ai_data.read().ok()?;
        let info = data
            .side_info
            .iter()
            .find(|info| info.side.eq_ignore_ascii_case(side))?;
        let sets = [
            Self::science_names_from_skill_ids(
                info.skill_set_1.num_skills,
                &info.skill_set_1.skills,
            ),
            Self::science_names_from_skill_ids(
                info.skill_set_2.num_skills,
                &info.skill_set_2.skills,
            ),
            Self::science_names_from_skill_ids(
                info.skill_set_3.num_skills,
                &info.skill_set_3.skills,
            ),
            Self::science_names_from_skill_ids(
                info.skill_set_4.num_skills,
                &info.skill_set_4.skills,
            ),
            Self::science_names_from_skill_ids(
                info.skill_set_5.num_skills,
                &info.skill_set_5.skills,
            ),
        ];
        sets.iter().any(|set| !set.is_empty()).then_some(sets)
    }

    /// ZH general residual first sciences when parsed AIData is not loaded.
    pub(super) fn residual_general_skillsets(side: &str) -> Option<[Vec<String>; 5]> {
        use crate::game_logic::host_faction_skirmish_residual::{
            SIDE_AMERICA, SIDE_CHINA, SIDE_GLA, SKIRMISH_AI_SIDE_INFO_RESIDUAL,
        };
        if side.eq_ignore_ascii_case(SIDE_AMERICA)
            || side.eq_ignore_ascii_case(SIDE_CHINA)
            || side.eq_ignore_ascii_case(SIDE_GLA)
        {
            return None;
        }
        let info = SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|info| info.side.eq_ignore_ascii_case(side))?;
        let mut sets = [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        if !info.skill_set1_first.is_empty() {
            sets[0].push(info.skill_set1_first.to_string());
        }
        if !info.skill_set2_first.is_empty() {
            sets[1].push(info.skill_set2_first.to_string());
        }
        sets.iter().any(|set| !set.is_empty()).then_some(sets)
    }

    /// C++ `doUpgradesAndSkills` SideInfo walk: leftover AIData, then residual
    /// general skillsets, then the three-faction hardcoded tables.
    pub(super) fn live_side_skillsets(&self, game_logic: &GameLogic) -> [Vec<String>; 5] {
        let side = self
            .live_player_side(game_logic)
            .or_else(|| self.side_info_name().map(str::to_string));
        if let Some(side) = side.as_deref() {
            if let Some(sets) = Self::aidata_side_skillsets(side) {
                return sets;
            }
            if let Some(sets) = Self::residual_general_skillsets(side) {
                return sets;
            }
        }
        self.side_skillsets()
            .map(|set| set.iter().map(|name| (*name).to_string()).collect())
    }

    /// C++ `AISideInfo::m_baseDefenseStructure1` for this host team.
    pub(super) fn base_defense_structure(&self) -> Option<&'static str> {
        use crate::game_logic::host_faction_skirmish_residual::SKIRMISH_AI_SIDE_INFO_RESIDUAL;
        let side = self.side_info_name()?;
        SKIRMISH_AI_SIDE_INFO_RESIDUAL
            .iter()
            .find(|info| info.side == side)
            .map(|info| info.base_defense_structure1)
    }

    pub fn add_building(&mut self, template_name: &str, position: Vec3, max_rebuilds: u32) {
        let mut building = AIBuildingInfo::new(template_name.to_string(), position, max_rebuilds);
        // Explicit requests (tests / leftover script stamp) are priority pads,
        // not automatic SkirmishBuildList entries.
        building.automatic_build = false;
        building.is_priority = true;
        self.building_queue.push(building);
    }

    /// C++ `AISkirmishPlayer::newMap` layout slot: increment rebuilds so the
    /// first construction does not consume the last rebuild.
    pub(super) fn add_layout_building(
        &mut self,
        template_name: &str,
        position: Vec3,
        max_rebuilds: u32,
    ) {
        let mut building = AIBuildingInfo::new(template_name.to_string(), position, max_rebuilds);
        building.increment_num_rebuilds();
        self.building_queue.push(building);
    }

    /// C++ `TAiData::m_skirmishBaseDefenseExtraDistance` (Default/AIData.ini = 150).
    pub const SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 = 150.0;

    pub(super) fn reset_base_defense_fan(&mut self) {
        self.cur_front_base_defense = 0;
        self.cur_flank_base_defense = 0;
        self.cur_front_left_defense_angle = 0.0;
        self.cur_front_right_defense_angle = 0.0;
        self.cur_left_flank_left_defense_angle = 0.0;
        self.cur_left_flank_right_defense_angle = 0.0;
        self.cur_right_flank_left_defense_angle = 0.0;
        self.cur_right_flank_right_defense_angle = 0.0;
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefense(false)` — one front-fan slot.
    pub(super) fn queue_front_base_defense(&mut self, game_logic: Option<&GameLogic>) {
        let Some(defense) = self.base_defense_structure() else {
            return;
        };
        if let Some(position) = self.place_next_base_defense_structure(game_logic, defense, false) {
            self.add_layout_building(defense, position, UNLIMITED_REBUILDS);
        }
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefense` — script `SKIRMISH_BUILD_BASE_DEFENSE_*`.
    pub fn build_script_base_defense(
        &mut self,
        game_logic: Option<&GameLogic>,
        flank: bool,
    ) -> bool {
        let Some(defense) = self.base_defense_structure() else {
            return false;
        };
        self.build_script_base_defense_structure(game_logic, defense, flank)
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefenseStructure` — script
    /// `SKIRMISH_BUILD_STRUCTURE_FRONT/FLANK`.
    pub fn build_script_base_defense_structure(
        &mut self,
        game_logic: Option<&GameLogic>,
        thing_name: &str,
        flank: bool,
    ) -> bool {
        let Some(position) = self.place_next_base_defense_structure(game_logic, thing_name, flank)
        else {
            return false;
        };
        self.add_building(thing_name, position, UNLIMITED_REBUILDS);
        true
    }

    /// Approach goal for the defense fan.
    ///
    /// C++ `buildAIBaseDefenseStructure` (`AISkirmishPlayer.cpp:599-609`):
    /// closest Center/Flank/Backdoor waypoint, else enemy structure bounds
    /// center for front, else abort for flank.
    pub(super) fn approach_goal(
        &self,
        game_logic: Option<&GameLogic>,
        flank: bool,
    ) -> Option<Vec3> {
        if let Some(gl) = game_logic {
            if let Some(enemy_id) = self.enemy_player_id {
                if let Some(enemy) = gl.get_player(enemy_id) {
                    return Some(self.find_enemy_base_center(gl, enemy.team));
                }
            }
            for player_id in 0..8u32 {
                if player_id == self.player_id {
                    continue;
                }
                if let Some(player) = gl.get_player(player_id) {
                    if player.team != self.team && player.is_alive {
                        return Some(self.find_enemy_base_center(gl, player.team));
                    }
                }
            }
        }
        if flank {
            return None;
        }
        // C++ no-waypoint front fallback is enemy bounds; without an enemy,
        // `find_enemy_base_center` uses the opposite corner (`-base_center`).
        Some(-self.base_center)
    }

    /// C++ `AISkirmishPlayer::buildAIBaseDefenseStructure` (`AISkirmishPlayer.cpp:580-686`).
    ///
    /// Walks the left/right approach fan until a legal pad is found or the
    /// angle exceeds π/3.
    pub(super) fn place_next_base_defense_structure(
        &mut self,
        game_logic: Option<&GameLogic>,
        thing_name: &str,
        flank: bool,
    ) -> Option<Vec3> {
        loop {
            let goal = self.approach_goal(game_logic, flank)?;
            let mut offset_x = goal.x - self.base_center.x;
            let mut offset_z = goal.z - self.base_center.z;
            let length = (offset_x * offset_x + offset_z * offset_z).sqrt();
            if length > 0.001 {
                offset_x /= length;
                offset_z /= length;
            } else {
                offset_x = 1.0;
                offset_z = 0.0;
            }
            let defense_distance = self.base_radius + Self::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
            offset_x *= defense_distance;
            offset_z *= defense_distance;

            let structure_radius = 20.0;
            let base_circumference = 2.0 * std::f32::consts::PI * defense_distance.max(1.0);
            let angle_offset =
                2.0 * std::f32::consts::PI * (structure_radius * 4.0 / base_circumference);

            let angle = if flank {
                let selector = self.cur_flank_base_defense >> 1;
                if self.cur_flank_base_defense & 1 != 0 {
                    if selector & 1 != 0 {
                        self.cur_left_flank_right_defense_angle -= angle_offset;
                        self.cur_left_flank_right_defense_angle
                    } else {
                        let result = self.cur_left_flank_left_defense_angle;
                        self.cur_left_flank_left_defense_angle += angle_offset;
                        result
                    }
                } else if selector & 1 != 0 {
                    self.cur_right_flank_right_defense_angle -= angle_offset;
                    self.cur_right_flank_right_defense_angle
                } else {
                    let result = self.cur_right_flank_left_defense_angle;
                    self.cur_right_flank_left_defense_angle += angle_offset;
                    result
                }
            } else if self.cur_front_base_defense & 1 != 0 {
                self.cur_front_right_defense_angle -= angle_offset;
                self.cur_front_right_defense_angle
            } else {
                let result = self.cur_front_left_defense_angle;
                self.cur_front_left_defense_angle += angle_offset;
                result
            };

            if angle > std::f32::consts::PI / 3.0 {
                return None;
            }

            let s = angle.sin();
            let c = angle.cos();
            let mut build_pos = self.base_center;
            build_pos.x += offset_x * c - offset_z * s;
            build_pos.z += offset_z * c + offset_x * s;

            if flank {
                self.cur_flank_base_defense += 1;
            } else {
                self.cur_front_base_defense += 1;
            }

            let legal = game_logic
                .map(|gl| gl.is_location_legal_to_build(self.team, build_pos, thing_name))
                .unwrap_or(true);
            if legal {
                return Some(build_pos);
            }
        }
    }

    /// If a queued front-defense pad is illegal, walk the C++ fan for a new pad.
    pub(super) fn relocate_defense_if_illegal(&mut self, game_logic: &GameLogic, index: usize) {
        let Some(building) = self.building_queue.get(index) else {
            return;
        };
        let name = building.template_name.clone();
        let position = building.position;
        if self.base_defense_structure() != Some(name.as_str()) {
            return;
        }
        if game_logic.is_location_legal_to_build(self.team, position, &name) {
            return;
        }
        if let Some(next) = self.place_next_base_defense_structure(Some(game_logic), &name, false) {
            if let Some(building) = self.building_queue.get_mut(index) {
                building.position = next;
            }
        }
    }

    /// Set up initial AI strategy based on personality
    pub(super) fn setup_initial_strategy(&mut self) {
        self.current_strategy = AIStrategy::EarlyGame;
        self.build_phase = AIBuildPhase::BaseConstruction;

        // Retail AIData StructureSeconds=0 / TeamSeconds=10, scaled by difficulty.
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_building_time =
            self.last_update_time + (Self::STRUCTURE_SECONDS * delay_modifier);
        self.next_team_time = self.last_update_time + (self.team_seconds * delay_modifier);
    }

    /// C++ `AISkirmishPlayer::acquireEnemy` (`AISkirmishPlayer.cpp:461-522`).
    pub(super) fn update_enemy_assessment(
        &mut self,
        game_logic: &mut GameLogic,
        current_time: f32,
    ) {
        // C++ `getAiEnemy` only calls this every 5s.
        if current_time - self.enemy_check_time < 5.0 {
            return;
        }
        self.enemy_check_time = current_time;

        if let Some(enemy_id) = self.enemy_player_id {
            if let Some(enemy) = game_logic.get_player(enemy_id) {
                if enemy.is_alive
                    && self.player_is_enemy(game_logic, enemy)
                    && !self.player_in_bad_shape(game_logic, enemy)
                {
                    return;
                }
            }
        }

        let mut best_enemy: Option<u32> = None;
        let mut best_distance_sqr = HUGE_DIST * HUGE_DIST;

        let player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        for player_id in player_ids {
            if player_id == self.player_id {
                continue;
            }
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            if !player.is_alive || !self.player_is_enemy(game_logic, player) {
                continue;
            }
            if !self.player_has_any_objects(game_logic, player.team) {
                continue;
            }

            let in_bad_shape = self.player_in_bad_shape(game_logic, player);
            let (min_x, min_z, max_x, max_z) =
                self.player_structure_bounds(game_logic, player.team);
            let enemy_center = if min_x == 0.0 && min_z == 0.0 && max_x == 0.0 && max_z == 0.0 {
                self.find_enemy_base_center(game_logic, player.team)
            } else {
                Vec3::new(
                    min_x + (max_x - min_x) * 0.5,
                    0.0,
                    min_z + (max_z - min_z) * 0.5,
                )
            };
            let dx = enemy_center.x - self.base_center.x;
            let dz = enemy_center.z - self.base_center.z;
            let mut dist_sqr = dx * dx + dz * dz;
            if in_bad_shape {
                dist_sqr = HUGE_DIST * HUGE_DIST * 0.5;
            }
            for &(other_id, other_target) in &self.peer_ai_targets {
                if other_id == self.player_id || other_id == player_id {
                    continue;
                }
                if other_target == Some(player_id) {
                    dist_sqr += 500.0 * 500.0;
                }
                if other_target == Some(self.player_id) {
                    dist_sqr -= 25.0 * 25.0;
                    if dist_sqr < 0.0 {
                        dist_sqr = 0.0;
                    }
                }
            }
            if dist_sqr < best_distance_sqr {
                best_distance_sqr = dist_sqr;
                best_enemy = Some(player_id);
            }
        }

        // C++ only replaces when bestEnemy != NULL && bestEnemy != m_currentEnemy.
        let Some(best) = best_enemy else {
            return;
        };
        if self.enemy_player_id == Some(best) {
            return;
        }
        self.enemy_player_id = Some(best);
        log::debug!(
            "AI Player {} ({}) targeting enemy Player {}",
            self.player_id,
            self.team.get_name(),
            best
        );
    }

    pub(super) fn player_is_enemy(&self, _game_logic: &GameLogic, player: &Player) -> bool {
        if player.team == self.team {
            return false;
        }
        if player.alliance_team >= 0 {
            if let Some(me) = _game_logic.get_player(self.player_id) {
                if me.alliance_team >= 0 && me.alliance_team == player.alliance_team {
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn player_has_any_objects(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic
            .host_objects()
            .values()
            .any(|object| object.team == team && object.is_alive())
    }

    pub(super) fn player_has_any_units(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == team
                && object.is_alive()
                && (object.is_kind_of(KindOf::Infantry)
                    || object.is_kind_of(KindOf::Vehicle)
                    || object.is_kind_of(KindOf::Aircraft))
        })
    }

    pub(super) fn player_has_any_build_facility(&self, game_logic: &GameLogic, team: Team) -> bool {
        game_logic.host_objects().values().any(|object| {
            object.team == team
                && object.is_alive()
                && (object.is_kind_of(KindOf::CommandCenter)
                    || object.is_kind_of(KindOf::FSBarracks)
                    || object.is_kind_of(KindOf::FSWarFactory)
                    || object.is_kind_of(KindOf::FSAirfield))
        })
    }

    pub(super) fn player_in_bad_shape(&self, game_logic: &GameLogic, player: &Player) -> bool {
        !self.player_has_any_units(game_logic, player.team)
            || !self.player_has_any_build_facility(game_logic, player.team)
    }
}
