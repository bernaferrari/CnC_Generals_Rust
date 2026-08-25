use super::*;

/// AI Manager coordinates all AI players
#[derive(Debug)]
pub struct AIManager {
    pub ai_players: HashMap<u32, AIPlayer>,
    pub update_interval: f32,
    pub last_update_time: f32,
}

impl Default for AIManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AIManager {
    /// Create new AI manager
    pub fn new() -> Self {
        Self {
            ai_players: HashMap::new(),
            update_interval: 1.0 / 30.0, // C++ AI::update every logic frame (30 Hz)
            // Negative so the first host update at sim_time=0 is not skipped.
            last_update_time: -1.0,
        }
    }

    /// Add AI player
    pub fn add_ai_player(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        let mut ai_player = AIPlayer::new(player_id, team, difficulty);

        // Initialize with team-appropriate base position
        // Keep pads inside default 512×512 world with MinDistFromEdgeOfMapForBuild=30
        // and layout offsets up to +100 (WarFactory). |center|<=120 → max pad 220 < 226.
        let base_position = match team {
            Team::USA => Vec3::new(-120.0, 0.0, -120.0),
            Team::China => Vec3::new(120.0, 0.0, -120.0),
            Team::GLA => Vec3::new(120.0, 0.0, 120.0),
            _ => Vec3::ZERO,
        };

        ai_player.initialize(base_position);
        self.ai_players.insert(player_id, ai_player);

        log::info!(
            "Added AI player {} ({}) with {} difficulty",
            player_id,
            team.get_name(),
            match difficulty {
                AIDifficulty::Easy => "Easy",
                AIDifficulty::Medium => "Medium",
                AIDifficulty::Hard => "Hard",
                AIDifficulty::Brutal => "Brutal",
            }
        );
    }

    /// C++ `AIPlayer` ctor `p->setCanBuildUnits(false)`.
    pub fn apply_ctor_can_build_units(game_logic: &mut GameLogic, player_id: u32) {
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(false);
        }
    }

    /// Update all AI players
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.last_update_time >= 0.0
            && current_time - self.last_update_time < self.update_interval
        {
            return;
        }

        let peer_targets: Vec<(u32, Option<u32>)> = self
            .ai_players
            .iter()
            .map(|(&id, ai)| (id, ai.enemy_player_id))
            .collect();
        let destroyed = gamelogic::team::take_host_pre_team_destroy_requests();
        let player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        for player_id in player_ids {
            if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
                for (team_id, team_name) in &destroyed {
                    ai_player.ai_pre_team_destroy(Some(*team_id), team_name);
                }
                ai_player.peer_ai_targets = peer_targets.clone();
                ai_player.update(game_logic, current_time);
            }
        }

        self.last_update_time = current_time;
    }

    /// Set AI difficulty for a player
    pub fn set_difficulty(&mut self, player_id: u32, difficulty: AIDifficulty) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.difficulty = difficulty;
        }
    }

    /// Relocate one AI player's base/layout without removing templates.
    pub fn relocate_ai_base(&mut self, player_id: u32, base_position: Vec3) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.relocate_base(base_position);
            log::info!(
                "AI Manager: relocated player {} base to {:?}",
                player_id,
                base_position
            );
        }
    }

    /// Enable/disable AI for a player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.is_active = active;
        }
    }

    /// Capture the bounded, decision-relevant host-AI state that can safely
    /// survive a map/object restore.  Pathfinder jobs, production pointers,
    /// and attack targets deliberately are not serialized: their object
    /// references are transient and are rebuilt after the snapshot is loaded.
    pub fn snapshot_players_for_save(&self) -> Vec<crate::save_load::AIPlayerSnapshot> {
        let mut player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        player_ids.sort_unstable();

        player_ids
            .into_iter()
            .filter_map(|player_id| self.ai_players.get(&player_id))
            .map(|ai| {
                let defensive_groups = (!ai.defensive_units.is_empty())
                    .then(|| crate::save_load::AIUnitGroupSnapshot {
                        group_id: ai.player_id,
                        units: ai.defensive_units.clone(),
                        role: "Defensive".to_string(),
                        current_task: "GuardBase".to_string(),
                        formation: "Default".to_string(),
                        target_position: Some(ai.base_center),
                    })
                    .into_iter()
                    .collect();

                crate::save_load::AIPlayerSnapshot {
                    player_id: ai.player_id,
                    difficulty: Self::difficulty_name(ai.difficulty).to_string(),
                    personality: Self::personality_name(ai.personality).to_string(),
                    current_strategy: Self::strategy_name(ai.current_strategy).to_string(),
                    is_active: ai.is_active,
                    base_center: Some(ai.base_center),
                    base_radius: ai.base_radius,
                    activity_count: ai.activity_count,
                    strategic_state: crate::save_load::AIStrategicStateSnapshot {
                        current_phase: Self::build_phase_name(ai.build_phase).to_string(),
                        objectives: Vec::new(),
                        threat_assessment: crate::save_load::ThreatAssessmentSnapshot {
                            enemy_strengths: HashMap::new(),
                            vulnerable_areas: Vec::new(),
                            threat_level: 0.0,
                        },
                    },
                    tactical_state: crate::save_load::AITacticalStateSnapshot {
                        unit_groups: defensive_groups,
                        active_attacks: Vec::new(),
                        defensive_positions: vec![ai.base_center],
                    },
                    // The host AI rebuild queue carries live object/factory
                    // references.  It is intentionally regenerated after a
                    // load rather than persisted with stale IDs.
                    economic_state: crate::save_load::AIEconomicStateSnapshot {
                        build_priorities: Vec::new(),
                        economic_focus: String::new(),
                        resource_allocation: crate::save_load::ResourceAllocation {
                            military_percentage: 0.0,
                            economic_percentage: 0.0,
                            defensive_percentage: 0.0,
                        },
                    },
                }
            })
            .collect()
    }

    /// Recreate registered host-AI players from an offline snapshot.
    ///
    /// The caller supplies restored player teams because save rows identify an
    /// AI by player id, while team ownership remains part of `PlayerSnapshot`.
    /// Empty snapshots are handled by the caller as the legacy fallback case.
    pub fn restore_players_from_save(
        &mut self,
        snapshots: &[crate::save_load::AIPlayerSnapshot],
        player_teams: &HashMap<u32, Team>,
    ) {
        let mut rows: Vec<_> = snapshots.iter().collect();
        rows.sort_by_key(|snapshot| snapshot.player_id);
        self.ai_players.clear();
        let mut restored_ids = HashSet::new();

        for snapshot in rows {
            if !restored_ids.insert(snapshot.player_id) {
                log::warn!(
                    "Ignoring duplicate host AI snapshot for player {}",
                    snapshot.player_id
                );
                continue;
            }
            let Some(&team) = player_teams.get(&snapshot.player_id) else {
                log::warn!(
                    "Ignoring host AI snapshot for missing player {}",
                    snapshot.player_id
                );
                continue;
            };
            if team == Team::Neutral {
                log::warn!(
                    "Ignoring host AI snapshot for neutral player {}",
                    snapshot.player_id
                );
                continue;
            }

            let difficulty =
                Self::difficulty_from_name(&snapshot.difficulty).unwrap_or(AIDifficulty::Medium);
            self.add_ai_player(snapshot.player_id, team, difficulty);
            let Some(ai) = self.ai_players.get_mut(&snapshot.player_id) else {
                continue;
            };

            ai.personality = Self::personality_from_name(&snapshot.personality)
                .unwrap_or_else(|| AIPersonality::for_team(team));
            ai.is_active = snapshot.is_active;
            // Legacy snapshot rows represented the same anchor only as the
            // first tactical defensive position; prefer the dedicated field
            // when a current-format save has it.
            let saved_base_center = snapshot
                .base_center
                .or_else(|| snapshot.tactical_state.defensive_positions.first().copied());
            if let Some(base_center) = saved_base_center.filter(|pos| pos.is_finite()) {
                ai.relocate_base(base_center);
            }
            if snapshot.base_radius.is_finite() && snapshot.base_radius > 0.0 {
                ai.base_radius = snapshot.base_radius;
            }
            ai.current_strategy = Self::strategy_from_name(&snapshot.current_strategy)
                .unwrap_or(AIStrategy::EarlyGame);
            ai.build_phase = Self::build_phase_from_name(&snapshot.strategic_state.current_phase)
                .unwrap_or(AIBuildPhase::BaseConstruction);
            ai.activity_count = snapshot.activity_count;
            ai.defensive_units = snapshot
                .tactical_state
                .unit_groups
                .iter()
                .filter(|group| group.role.eq_ignore_ascii_case("defensive"))
                .flat_map(|group| group.units.iter().copied())
                .collect();

            // A target can be destroyed/reused during object restoration.  Do
            // not revive a half-resolved attack or production pointer; fresh
            // host AI evaluation will issue legal actions on the next update.
            ai.attack_in_progress = false;
            ai.team_queue.clear();
            ai.team_ready_queue.clear();
            ai.structures_to_repair.clear();
            ai.repair_dozer = None;
            ai.dozer_queued_for_repair = false;
            ai.dozer_is_repairing = false;
            ai.skillset_selector = INVALID_SKILLSET_SELECTION;
            ai.last_update_time = 0.0;
            ai.resource_check_time = 0.0;
            ai.enemy_check_time = 0.0;
            ai.next_building_time = 0.0;
            ai.next_team_queue_time = 0.0;
            ai.next_team_time = 0.0;
        }

        // Let the first post-load logic frame rebuild actions immediately.
        self.last_update_time = -1.0;
    }

    pub fn capture_queue_persist(
        &self,
    ) -> Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist> {
        let mut player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        player_ids.sort_unstable();
        player_ids
            .into_iter()
            .filter_map(|player_id| self.ai_players.get(&player_id))
            .map(AIPlayer::capture_queue_persist)
            .collect()
    }

    pub fn apply_queue_persist(
        &mut self,
        rows: Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist>,
    ) {
        for row in rows {
            let Some(ai) = self.ai_players.get_mut(&row.player_id) else {
                continue;
            };
            ai.apply_queue_persist(row);
        }
    }

    pub fn clear_queue_persist(&mut self) {
        for ai in self.ai_players.values_mut() {
            ai.clear_queue_persist();
        }
    }

    fn difficulty_name(value: AIDifficulty) -> &'static str {
        match value {
            AIDifficulty::Easy => "Easy",
            AIDifficulty::Medium => "Medium",
            AIDifficulty::Hard => "Hard",
            AIDifficulty::Brutal => "Brutal",
        }
    }

    fn difficulty_from_name(value: &str) -> Option<AIDifficulty> {
        if value.eq_ignore_ascii_case("easy") {
            Some(AIDifficulty::Easy)
        } else if value.eq_ignore_ascii_case("medium") {
            Some(AIDifficulty::Medium)
        } else if value.eq_ignore_ascii_case("hard") {
            Some(AIDifficulty::Hard)
        } else if value.eq_ignore_ascii_case("brutal") {
            Some(AIDifficulty::Brutal)
        } else {
            None
        }
    }

    fn personality_name(value: AIPersonality) -> &'static str {
        match value {
            AIPersonality::Balanced => "Balanced",
            AIPersonality::Aggressive => "Aggressive",
            AIPersonality::Defensive => "Defensive",
            AIPersonality::Economic => "Economic",
            AIPersonality::Rush => "Rush",
        }
    }

    fn personality_from_name(value: &str) -> Option<AIPersonality> {
        if value.eq_ignore_ascii_case("balanced") {
            Some(AIPersonality::Balanced)
        } else if value.eq_ignore_ascii_case("aggressive") {
            Some(AIPersonality::Aggressive)
        } else if value.eq_ignore_ascii_case("defensive") {
            Some(AIPersonality::Defensive)
        } else if value.eq_ignore_ascii_case("economic") {
            Some(AIPersonality::Economic)
        } else if value.eq_ignore_ascii_case("rush") {
            Some(AIPersonality::Rush)
        } else {
            None
        }
    }

    fn strategy_name(value: AIStrategy) -> &'static str {
        match value {
            AIStrategy::EarlyGame => "EarlyGame",
            AIStrategy::MidGame => "MidGame",
            AIStrategy::LateGame => "LateGame",
            AIStrategy::Desperate => "Desperate",
        }
    }

    fn strategy_from_name(value: &str) -> Option<AIStrategy> {
        if value.eq_ignore_ascii_case("earlygame") {
            Some(AIStrategy::EarlyGame)
        } else if value.eq_ignore_ascii_case("midgame") {
            Some(AIStrategy::MidGame)
        } else if value.eq_ignore_ascii_case("lategame") {
            Some(AIStrategy::LateGame)
        } else if value.eq_ignore_ascii_case("desperate") {
            Some(AIStrategy::Desperate)
        } else {
            None
        }
    }

    fn build_phase_name(value: AIBuildPhase) -> &'static str {
        match value {
            AIBuildPhase::BaseConstruction => "BaseConstruction",
            AIBuildPhase::UnitProduction => "UnitProduction",
            AIBuildPhase::Expansion => "Expansion",
            AIBuildPhase::MassProduction => "MassProduction",
        }
    }

    fn build_phase_from_name(value: &str) -> Option<AIBuildPhase> {
        if value.eq_ignore_ascii_case("baseconstruction") {
            Some(AIBuildPhase::BaseConstruction)
        } else if value.eq_ignore_ascii_case("unitproduction") {
            Some(AIBuildPhase::UnitProduction)
        } else if value.eq_ignore_ascii_case("expansion") {
            Some(AIBuildPhase::Expansion)
        } else if value.eq_ignore_ascii_case("massproduction") {
            Some(AIBuildPhase::MassProduction)
        } else {
            None
        }
    }

    /// Sum of production-linked AI actions across all host AI players.
    pub fn total_activity_count(&self) -> u64 {
        self.ai_players.values().map(|p| p.activity_count).sum()
    }

    /// Get AI player information
    pub fn get_ai_info(&self, player_id: u32) -> Option<String> {
        self.ai_players.get(&player_id).map(|ai_player| format!(
                "AI Player {} ({}): {:?} difficulty, {:?} strategy, {} buildings queued, {} teams queued",
                player_id,
                ai_player.team.get_name(),
                ai_player.difficulty,
                ai_player.current_strategy,
                ai_player.building_queue.len(),
                ai_player.team_queue.len()
            ))
    }

    /// Return the most common configured difficulty across active AI players.
    ///
    /// Ties are resolved towards the harder difficulty to better represent
    /// gameplay pressure in mixed-difficulty skirmishes.
    pub fn dominant_difficulty(&self) -> Option<AIDifficulty> {
        if self.ai_players.is_empty() {
            return None;
        }

        let mut counts = [0usize; 4]; // Easy, Medium, Hard, Brutal
        for ai_player in self.ai_players.values() {
            let idx = match ai_player.difficulty {
                AIDifficulty::Easy => 0,
                AIDifficulty::Medium => 1,
                AIDifficulty::Hard => 2,
                AIDifficulty::Brutal => 3,
            };
            counts[idx] += 1;
        }

        let mut best_idx = 0usize;
        for idx in 1..counts.len() {
            if counts[idx] > counts[best_idx] || (counts[idx] == counts[best_idx] && idx > best_idx)
            {
                best_idx = idx;
            }
        }

        Some(match best_idx {
            0 => AIDifficulty::Easy,
            1 => AIDifficulty::Medium,
            2 => AIDifficulty::Hard,
            _ => AIDifficulty::Brutal,
        })
    }

    /// True when a host AI player is registered and marked active.
    pub fn is_ai_active(&self, player_id: u32) -> bool {
        self.ai_players
            .get(&player_id)
            .map(|p| p.is_active)
            .unwrap_or(false)
    }

    /// Configured difficulty for a registered host AI player.
    pub fn ai_difficulty(&self, player_id: u32) -> Option<AIDifficulty> {
        self.ai_players.get(&player_id).map(|p| p.difficulty)
    }

    /// Teams of all registered host AI players (for template rebind).
    pub fn registered_teams(&self) -> Vec<Team> {
        let mut teams = Vec::new();
        for ai in self.ai_players.values() {
            if !teams.contains(&ai.team) {
                teams.push(ai.team);
            }
        }
        teams
    }

    /// Rebind host AI after world objects were wiped (map load / preserve path).
    ///
    /// Keeps registration, difficulty, `is_active`, personality, and base layout
    /// template names. Drops stale object/factory IDs so rebuild soup can run
    /// again, and reopens early-base timers. Remaining rebuilds
    /// (`AIBuildingInfo.rebuild_count`) stay — leftover `BuildListInfo.num_rebuilds`
    /// is persisted, not reset on rebind.
    pub fn rebind_after_world_reset(&mut self) {
        log::info!(
            "AI Manager: rebinding {} AI player(s) after world reset",
            self.ai_players.len()
        );
        for ai_player in self.ai_players.values_mut() {
            for building in &mut ai_player.building_queue {
                // Map load clears objects; this is not a combat loss. Keep remaining rebuilds.
                building.object_id = None;
                building.is_built = false;
            }
            for team in &mut ai_player.team_queue {
                team.completed = false;
                for order in &mut team.work_orders {
                    order.factory_id = None;
                    order.queued_count = 0;
                    order.num_completed = 0;
                    order.observed_unit_ids.clear();
                }
            }
            ai_player.defensive_units.clear();
            ai_player.attack_in_progress = false;
            // Timing: allow next host AI tick to act immediately.
            ai_player.last_update_time = 0.0;
            ai_player.resource_check_time = 0.0;
            ai_player.enemy_check_time = 0.0;
            ai_player.next_building_time = 0.0;
            ai_player.next_team_queue_time = 0.0;
            ai_player.next_team_time = 0.0;
            ai_player.last_attack_time = 0.0;
            log::debug!(
                "  Rebound AI player {} ({}) active={} difficulty={:?}",
                ai_player.player_id,
                ai_player.team.get_name(),
                ai_player.is_active,
                ai_player.difficulty
            );
        }
        // Negative so the first post-load host update is not rate-limited away.
        self.last_update_time = -1.0;
    }

    /// Called when a game is loaded from save
    pub fn on_game_loaded(&mut self) {
        log::info!("AI Manager: Game loaded, reinitializing AI state...");
        // Save restore also wipes live object pointers in practice; share map-load rebind.
        self.rebind_after_world_reset();
        log::info!("AI Manager: Game load initialization complete");
    }

    pub fn resolve_player_id(game_logic: &GameLogic, token: &str) -> Option<u32> {
        let t = token.trim();
        if t.is_empty() {
            return None;
        }
        if let Ok(id) = t.parse::<u32>() {
            if game_logic.get_player(id).is_some() {
                return Some(id);
            }
        }
        game_logic.get_players().iter().find_map(|(id, player)| {
            let team_name = player.team.get_name();
            let lower = t.to_ascii_lowercase();
            if player.name.eq_ignore_ascii_case(t)
                || team_name.eq_ignore_ascii_case(t)
                || lower.contains(&team_name.to_ascii_lowercase())
            {
                Some(*id)
            } else {
                None
            }
        })
    }

    /// C++ `AIPlayer::buildSpecificAITeam` live host entry.
    pub fn build_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_specific_ai_team(game_logic, team_name, priority_build))
    }

    /// Resolve prototype owner then `buildSpecificAITeam(..., true)`.
    pub fn build_specific_ai_team_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        team_name: &str,
        priority_build: bool,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_specific_ai_team(game_logic, id, team_name, priority_build)
    }

    /// C++ `AIPlayer::recruitSpecificAITeam` live host entry.
    pub fn recruit_specific_ai_team(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.recruit_specific_ai_team(game_logic, team_name, recruit_radius))
    }

    /// Resolve prototype owner then `recruitSpecificAITeam`.
    pub fn recruit_specific_ai_team_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        team_name: &str,
        recruit_radius: f32,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.recruit_specific_ai_team(game_logic, id, team_name, recruit_radius)
    }

    /// C++ `ScriptActions::doGuardSupplyCenter` → `AIPlayer::guardSupplyCenter`.
    pub fn guard_supply_center_for_team(
        &mut self,
        game_logic: &mut GameLogic,
        team_name: &str,
        min_supplies: i32,
    ) -> bool {
        let Some(player_id) = self.resolve_guard_supply_player(game_logic, team_name) else {
            return false;
        };
        let Some(ai) = self.ai_players.get_mut(&player_id) else {
            return false;
        };
        ai.guard_supply_center(game_logic, team_name, min_supplies);
        true
    }

    fn resolve_guard_supply_player(&self, game_logic: &GameLogic, team_name: &str) -> Option<u32> {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(prototype) = factory.find_team_prototype(team_name) {
                let owner = prototype.get_owner_name().to_string();
                if !owner.is_empty() {
                    if let Some(id) = Self::resolve_player_id(game_logic, &owner) {
                        if self.ai_players.contains_key(&id) {
                            return Some(id);
                        }
                    }
                }
            }
        }
        let needle = team_name.trim();
        if !needle.is_empty() {
            for obj in game_logic.host_objects().values() {
                if !obj.is_alive()
                    || obj.team_instance_name.is_empty()
                    || !obj.team_instance_name.eq_ignore_ascii_case(needle)
                {
                    continue;
                }
                if let Some((&id, _)) = game_logic
                    .get_players()
                    .iter()
                    .find(|(_, player)| player.team == obj.team)
                {
                    if self.ai_players.contains_key(&id) {
                        return Some(id);
                    }
                }
            }
        }
        Self::resolve_player_id(game_logic, team_name).filter(|id| self.ai_players.contains_key(id))
    }

    /// C++ `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST` live host entry.
    pub fn fire_skirmish_special_power_at_most_cost(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        power_name: &str,
    ) {
        let Some(player_id) = Self::resolve_player_id(game_logic, player_token)
            .or_else(|| {
                self.ai_players
                    .keys()
                    .copied()
                    .find(|id| Self::resolve_player_id(game_logic, player_token) == Some(*id))
            })
            .or_else(|| {
                // Token may name a team the AI owns even if Player.name differs.
                self.ai_players.iter().find_map(|(id, ai)| {
                    let team = ai.team.get_name();
                    player_token
                        .to_ascii_lowercase()
                        .contains(&team.to_ascii_lowercase())
                        .then_some(*id)
                })
            })
        else {
            return;
        };
        if let Some(ai) = self.ai_players.get_mut(&player_id) {
            ai.fire_named_special_power(game_logic, power_name);
        }
    }

    /// C++ `SKIRMISH_BUILD_BUILDING` live host entry.
    pub fn build_specific_ai_building(&mut self, player_id: u32, thing_name: &str) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_specific_ai_building(thing_name))
    }

    pub fn build_specific_ai_building_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self.build_specific_ai_building(id, thing_name);
        }
        // Script often omits player and stamps the current skirmish AI.
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter()
            .any(|id| self.build_specific_ai_building(id, thing_name))
    }

    /// C++ `Player::buildBaseDefense` live host entry (current skirmish AI).
    pub fn build_ai_base_defense_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        flank: bool,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self
                .ai_players
                .get_mut(&id)
                .is_some_and(|ai| ai.build_script_base_defense(Some(game_logic), flank));
        }
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter().any(|id| {
            self.ai_players
                .get_mut(&id)
                .is_some_and(|ai| ai.build_script_base_defense(Some(game_logic), flank))
        })
    }

    /// C++ `Player::buildBaseDefenseStructure` live host entry.
    pub fn build_ai_base_defense_structure_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
        flank: bool,
    ) -> bool {
        if let Some(id) = Self::resolve_player_id(game_logic, player_token) {
            return self.ai_players.get_mut(&id).is_some_and(|ai| {
                ai.build_script_base_defense_structure(Some(game_logic), thing_name, flank)
            });
        }
        let ids: Vec<u32> = self.ai_players.keys().copied().collect();
        ids.into_iter().any(|id| {
            self.ai_players.get_mut(&id).is_some_and(|ai| {
                ai.build_script_base_defense_structure(Some(game_logic), thing_name, flank)
            })
        })
    }

    /// C++ `AIPlayer::buildBySupplies` live host entry.
    pub fn build_by_supplies(
        &mut self,
        game_logic: &GameLogic,
        player_id: u32,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_by_supplies(game_logic, minimum_cash, thing_name))
    }

    /// C++ `AIPlayer::buildBySupplies` for a script player token.
    pub fn build_by_supplies_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        minimum_cash: i32,
        thing_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_by_supplies(game_logic, id, minimum_cash, thing_name)
    }

    /// C++ `AIPlayer::buildUpgrade` live host entry.
    pub fn build_upgrade(
        &mut self,
        game_logic: &mut GameLogic,
        player_id: u32,
        upgrade_name: &str,
    ) -> bool {
        self.ai_players
            .get_mut(&player_id)
            .is_some_and(|ai| ai.build_upgrade(game_logic, upgrade_name))
    }

    pub fn build_upgrade_for_token(
        &mut self,
        game_logic: &mut GameLogic,
        player_token: &str,
        upgrade_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_upgrade(game_logic, id, upgrade_name)
    }

    /// C++ `AIPlayer::buildSpecificBuildingNearestTeam` live host entry.
    pub fn build_specific_building_nearest_team(
        &mut self,
        game_logic: &GameLogic,
        player_id: u32,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        self.ai_players.get_mut(&player_id).is_some_and(|ai| {
            ai.build_specific_building_nearest_team(game_logic, thing_name, team_name)
        })
    }

    pub fn build_specific_building_nearest_team_for_token(
        &mut self,
        game_logic: &GameLogic,
        player_token: &str,
        thing_name: &str,
        team_name: &str,
    ) -> bool {
        let Some(id) = Self::resolve_player_id(game_logic, player_token) else {
            return false;
        };
        self.build_specific_building_nearest_team(game_logic, id, thing_name, team_name)
    }

    /// Clear all pending AI commands
    pub fn clear_pending_commands(&mut self) {
        log::info!("AI Manager: Clearing all pending commands...");

        for ai_player in self.ai_players.values_mut() {
            // Clear building queues
            ai_player.building_queue.clear();

            // Clear team queues
            ai_player.team_queue.clear();

            // Reset attack state
            ai_player.attack_in_progress = false;

            log::debug!(
                "  Cleared commands for AI player {} ({})",
                ai_player.player_id,
                ai_player.team.get_name()
            );
        }

        log::info!("AI Manager: All pending commands cleared");
    }
}
