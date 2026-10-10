use super::*;
use std::collections::BTreeMap;

/// AI Manager coordinates all AI players
#[derive(Debug)]
pub struct AIManager {
    /// C++ PlayerList executes by numeric slot; retain that order in storage.
    pub ai_players: BTreeMap<u32, AIPlayer>,
    team_factory: gamelogic::team::TeamFactoryHandle,
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
            ai_players: BTreeMap::new(),
            team_factory: gamelogic::team::TeamFactoryHandle::new(),
        }
    }

    pub(crate) fn with_team_factory(team_factory: gamelogic::team::TeamFactoryHandle) -> Self {
        Self {
            team_factory,
            ai_players: BTreeMap::new(),
        }
    }

    /// Add AI player
    pub fn add_ai_player(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        let mut ai_player =
            AIPlayer::new_with_team_factory(player_id, team, difficulty, self.team_factory.clone());

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

    /// C++ `AISkirmishPlayer` ctor `p->setCanBuildUnits(true)`
    /// (AISkirmishPlayer.cpp:59-62): the skirmish subclass re-enables unit
    /// production that the base `AIPlayer` ctor disabled.  Without it the
    /// skirmish team path (`should_build_new_team`) never runs.
    pub fn apply_skirmish_can_build_units(game_logic: &mut GameLogic, player_id: u32) {
        if let Some(player) = game_logic.get_player_mut(player_id) {
            player.set_can_build_units(true);
        }
    }

    /// Update all AI players
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let ai_data = AiDataView::from_world(game_logic);
        Self::update_players(&mut self.ai_players, game_logic, current_time, &ai_data);
    }

    /// Borrow the driving match through player callbacks without constructing
    /// a replacement manager/team factory on every frame. Player state remains
    /// owned by the same manager before and after the synchronous pass.
    pub(crate) fn update_owned(game_logic: &mut GameLogic, current_time: f32) {
        let ai_data = AiDataView::from_world(game_logic);
        let mut players = std::mem::take(&mut game_logic.ai_manager.ai_players);
        Self::update_players(&mut players, game_logic, current_time, &ai_data);
        game_logic.ai_manager.ai_players = players;
    }

    fn update_players(
        players: &mut BTreeMap<u32, AIPlayer>,
        game_logic: &mut GameLogic,
        current_time: f32,
        ai_data: &AiDataView,
    ) {
        // GameLogic calls the AI once per advanced fixed logic frame, matching
        // C++ GameLogic::update -> TheAI->UPDATE. Do not re-gate that cadence
        // with accumulated f32 seconds: rounding can silently skip a frame.
        // C++ PlayerList.cpp:221-228 visits ascending player slots. Commands,
        // object admissions and random draws must not depend on HashMap order.
        let mut peer_targets: Vec<(u32, Option<u32>)> = players
            .iter()
            .map(|(&id, ai)| (id, ai.enemy_player_id))
            .collect();
        let destroyed = game_logic.take_ai_team_destroy_notifications();
        let scripts = gamelogic::scripting::engine::get_script_engine().clone();
        let mut ordered: Vec<_> = players.values_mut().collect();
        for slot in 0..ordered.len() {
            let (before, remaining) = ordered.split_at_mut(slot);
            let (ai_player, after) = remaining.split_first_mut().expect("current AI slot");
            for (team_id, team_name) in &destroyed {
                ai_player.ai_pre_team_destroy(Some(*team_id), team_name);
            }
            ai_player.peer_ai_targets = peer_targets.clone();
            let mut notify = |current: &mut AIPlayer, _world: &mut GameLogic, id, name: &str| {
                for peer in before.iter_mut() {
                    peer.ai_pre_team_destroy(Some(id), name);
                }
                current.ai_pre_team_destroy(Some(id), name);
                for peer in after.iter_mut() {
                    peer.ai_pre_team_destroy(Some(id), name);
                }
            };
            // Existing Core player-class compatibility input, resolved at the
            // outer phase. hq-tctn5 tracks admitting this class into Main state.
            let skirmish = ai_player.leftover_is_skirmish_ai();
            ai_player.update_with_ai_data_and_team_owner(
                game_logic,
                current_time,
                ai_data,
                skirmish,
                &scripts,
                &mut notify,
            );
            // C++ getCurrentEnemy observes earlier slots after their update,
            // and later slots before theirs, during this same synchronous pass.
            peer_targets[slot].1 = ai_player.enemy_player_id;
        }
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
        self.ai_players
            .values()
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
    /// Empty rows replace this manager's roster with no controllers. Both
    /// accepted world schemas serialize that absence explicitly.
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
    }

    pub fn capture_queue_persist(
        &self,
    ) -> Vec<crate::save_load::snapshot::ai_player_queue_persist::AIPlayerQueuePersist> {
        self.ai_players
            .values()
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
        // C++ ScriptEngine::getPlayerFromAsciiString resolves the exact player
        // name key. Scan in slot order so the answer never depends on HashMap
        // bucket order, and only fall back to a faction-name match when exactly
        // one player has that faction (same-faction players stay distinct).
        let mut players: Vec<(&u32, &crate::game_logic::Player)> =
            game_logic.get_players().iter().collect();
        players.sort_unstable_by_key(|(id, _)| **id);
        if let Some((id, _)) = players.iter().find(|(_, player)| {
            player.name.eq_ignore_ascii_case(t)
                || player.map_side.map_player_name.eq_ignore_ascii_case(t)
        }) {
            return Some(**id);
        }
        let lower = t.to_ascii_lowercase();
        let mut faction_matches = players.iter().filter(|(_, player)| {
            let team_name = player.team.get_name();
            team_name.eq_ignore_ascii_case(t) || lower.contains(&team_name.to_ascii_lowercase())
        });
        match (faction_matches.next(), faction_matches.next()) {
            (Some((id, _)), None) => Some(**id),
            _ => None,
        }
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
        if let Ok(factory) = game_logic.team_factory.lock() {
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

#[cfg(test)]
mod aidata_owner_tests {
    use super::*;
    use game_engine::common::ini::ini_ai_data::{
        self, AIDataStore, AiSideBuildList, BuildListEntry,
    };
    use glam::Vec3;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant};

    struct RestoreCommonStore {
        installed: Arc<RwLock<AIDataStore>>,
        previous: Option<Arc<RwLock<AIDataStore>>>,
    }

    impl RestoreCommonStore {
        fn foreign_sentinel() -> Self {
            let installed = Arc::new(RwLock::new(AIDataStore::default()));
            {
                let mut store = installed.write().unwrap();
                store.ensure_base();
                let data = store.get_active_mut().unwrap();
                data.max_recruit_distance = 9_999.0;
                data.rotate_skirmish_bases = true;
            }
            Self {
                installed,
                previous: None,
            }
        }

        fn activate(&mut self) {
            self.previous = ini_ai_data::install_ai_data_store(Arc::clone(&self.installed));
        }
    }

    impl Drop for RestoreCommonStore {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                ini_ai_data::install_ai_data_store(previous);
            } else {
                ini_ai_data::uninstall_ai_data_store_if_current(&self.installed);
            }
        }
    }

    // The Common active-store selector is process-wide. Keep this fixture in an exact child,
    // restore the prior selector with RAII, and require the named test to actually execute.
    fn run_isolated(test_name: &str) -> bool {
        const MARKER: &str = "GENERALS_AI_DATA_OWNER_TEST";
        let exact = module_path!().split_once("::").unwrap().1.to_owned() + "::" + test_name;
        if std::env::var(MARKER).as_deref() == Ok(exact.as_str()) {
            return false;
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([&exact, "--exact", "--test-threads=1", "--nocapture"])
            .env(MARKER, &exact)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let readers = pipes.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        });
        let deadline = Instant::now() + Duration::from_secs(25);
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break (status, false);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                break (child.wait().unwrap(), true);
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let output =
            readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
        assert!(!timed_out, "{exact} timed out: {output:?}");
        assert!(status.success(), "{exact}: {output:?}");
        assert!(
            output[0].contains("1 passed; 0 failed"),
            "child did not execute exact regression: {output:?}"
        );
        true
    }

    fn add_world_ai(world: &mut crate::game_logic::GameLogic, wf_offset: f32, team_resources: f32) {
        world.add_player(crate::game_logic::Player::new(
            1,
            Team::USA,
            "same-slot AI",
            false,
        ));
        let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::CommandCenter);
        world.templates.insert("AmericaCommandCenter".into(), cc);
        let mut wf = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        wf.add_kind_of(crate::game_logic::KindOf::Structure);
        world.templates.insert("AmericaWarFactory".into(), wf);

        let mut data = world.ai_definitions.data().clone();
        data.rotate_skirmish_bases = false;
        data.max_recruit_distance = wf_offset;
        data.team_resources_to_build = team_resources;
        data.side_build_lists.clear();
        let mut list = AiSideBuildList::new("America".into());
        list.entries.push(BuildListEntry {
            building_name: "CC".into(),
            template_name: "AmericaCommandCenter".into(),
            location: (0.0, 0.0),
            rebuilds: 0,
            angle_radians: 0.0,
            initially_built: false,
            rally_point_offset: (0.0, 0.0),
            automatically_build: true,
        });
        list.entries.push(BuildListEntry {
            building_name: "WF".into(),
            template_name: "AmericaWarFactory".into(),
            location: (wf_offset, 0.0),
            rebuilds: 1,
            angle_radians: 0.0,
            initially_built: false,
            rally_point_offset: (0.0, 0.0),
            automatically_build: true,
        });
        data.side_build_lists.push(list);
        world.set_ai_definition_base(data);

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.base_center = Vec3::new(-40.0, 0.0, -40.0);
        world.ai_manager.ai_players.insert(1, ai);
    }

    fn wf_position(world: &crate::game_logic::GameLogic) -> Vec3 {
        world.ai_manager.ai_players[&1]
            .building_queue
            .iter()
            .find(|entry| entry.template_name == "AmericaWarFactory")
            .expect("the world-owned SideBuildList creates its authored WF pad")
            .position
    }

    fn assert_position(actual: Vec3, expected: Vec3) {
        assert!(
            (actual.x - expected.x).abs() < 0.0001,
            "x: {actual:?} != {expected:?}"
        );
        assert!(
            (actual.y - expected.y).abs() < 0.0001,
            "y: {actual:?} != {expected:?}"
        );
        assert!(
            (actual.z - expected.z).abs() < 0.0001,
            "z: {actual:?} != {expected:?}"
        );
    }

    #[test]
    fn update_owned_uses_each_games_catalog_not_foreign_common_active_slot() {
        if run_isolated("update_owned_uses_each_games_catalog_not_foreign_common_active_slot") {
            return;
        }
        let mut foreign = RestoreCommonStore::foreign_sentinel();
        let mut first = crate::game_logic::GameLogic::new();
        let mut second = crate::game_logic::GameLogic::new();
        add_world_ai(&mut first, 80.0, 0.8);
        add_world_ai(&mut second, 140.0, 0.72);
        foreign.activate();
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &foreign.installed
        ));

        AIManager::update_owned(&mut first, 0.0);
        let first_position = wf_position(&first);
        AIManager::update_owned(&mut second, 0.0);
        let second_position = wf_position(&second);
        AIManager::update_owned(&mut first, 0.0);
        assert_position(first_position, Vec3::new(-96.56854, 0.0, 16.56854));
        assert_position(second_position, Vec3::new(-138.99495, 0.0, 58.99495));
        assert_position(wf_position(&first), first_position);
        assert_eq!(
            foreign
                .installed
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .max_recruit_distance,
            9_999.0
        );
        assert!(
            foreign
                .installed
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .rotate_skirmish_bases
        );
    }

    #[test]
    fn explicit_ai_data_target_keeps_interleaved_catalogs_separate() {
        if run_isolated("explicit_ai_data_target_keeps_interleaved_catalogs_separate") {
            return;
        }
        use game_engine::common::ini::INI;
        use game_engine::common::ini::ini_ai_data::{
            AIDataStore, install_ai_data_store, uninstall_ai_data_store_if_current,
        };
        use std::sync::{Arc, RwLock};

        // Run this test in an isolated/serial harness: it temporarily exercises the
        // same Common active-store selector used by legacy parser callers.
        let sentinel = Arc::new(RwLock::new(AIDataStore::default()));
        {
            let mut store = sentinel.write().unwrap();
            store.ensure_base();
            store.get_active_mut().unwrap().team_resources_to_build = 0.91;
        }
        let old = install_ai_data_store(Arc::clone(&sentinel));
        struct Restore(Arc<RwLock<AIDataStore>>, Option<Arc<RwLock<AIDataStore>>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                if let Some(old) = self.1.take() {
                    install_ai_data_store(old);
                } else {
                    uninstall_ai_data_store_if_current(&self.0);
                }
            }
        }
        let _restore = Restore(Arc::clone(&sentinel), old);

        let empty = Arc::new(RwLock::new(AIDataStore::default()));
        let mut ini = INI::new();
        ini.set_ai_data_store_target(Arc::clone(&empty));
        ini.with_inline_source("", |ini| ini.parse_current_file())
            .unwrap();
        assert!(empty.read().unwrap().get_active().is_none());
        assert_eq!(
            sentinel
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .team_resources_to_build,
            0.91,
            "an input without AIData must leave both explicit and active stores untouched"
        );

        let a = Arc::new(RwLock::new(AIDataStore::default()));
        let b = Arc::new(RwLock::new(AIDataStore::default()));
        let parse_into = |target: &Arc<RwLock<AIDataStore>>, text: &str| {
            let mut ini = INI::new();
            ini.set_ai_data_store_target(Arc::clone(target));
            ini.with_inline_source(text, |ini| ini.parse_current_file())
                .unwrap();
            assert_eq!(
                sentinel
                    .read()
                    .unwrap()
                    .get_active()
                    .unwrap()
                    .team_resources_to_build,
                0.91,
                "explicit parser must not mutate active/foreign Common store"
            );
        };

        parse_into(
            &a,
            "AIData\n TeamResourcesToStart 0.21\n MaxRecruitRadius 210\nEnd\n",
        );
        parse_into(
            &b,
            "AIData\n TeamResourcesToStart 0.72\n MaxRecruitRadius 720\nEnd\n",
        );
        // INI::with_inline_source is Overwrite, same as C++ initSubsystem loads.
        parse_into(&a, "AIData\n TeamResourcesToStart 0.31\nEnd\n");
        parse_into(&b, "AIData\n TeamResourcesToStart 0.83\nEnd\n");

        let a = a.read().unwrap();
        let a = a.get_active().unwrap();
        assert_eq!(a.team_resources_to_build, 0.31);
        assert_eq!(a.max_recruit_distance, 210.0);
        let b = b.read().unwrap();
        let b = b.get_active().unwrap();
        assert_eq!(b.team_resources_to_build, 0.83);
        assert_eq!(b.max_recruit_distance, 720.0);
    }

    #[test]
    fn ai_data_view_captures_world_catalog_before_callbacks() {
        if run_isolated("ai_data_view_captures_world_catalog_before_callbacks") {
            return;
        }
        let mut foreign = RestoreCommonStore::foreign_sentinel();
        let mut first = crate::game_logic::GameLogic::new();
        let mut second = crate::game_logic::GameLogic::new();
        add_world_ai(&mut first, 80.0, 0.8);
        add_world_ai(&mut second, 140.0, 0.72);
        foreign.activate();

        let first_view = super::super::world_view::AiDataView::from_world(&first);
        let second_view = super::super::world_view::AiDataView::from_world(&second);
        assert_eq!(AIPlayer::aidata_max_recruit_distance(&first_view), 80.0);
        assert_eq!(AIPlayer::aidata_max_recruit_distance(&second_view), 140.0);
        assert_eq!(AIPlayer::team_resources_to_start_frac(&first_view), 0.8);
        assert_eq!(AIPlayer::team_resources_to_start_frac(&second_view), 0.72);
        assert_eq!(
            foreign
                .installed
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .max_recruit_distance,
            9_999.0,
        );
    }

    #[test]
    fn reset_keeps_each_world_ai_catalog_and_restores_the_previous_selector() {
        if run_isolated("reset_keeps_each_world_ai_catalog_and_restores_the_previous_selector") {
            return;
        }

        let process_store = ini_ai_data::process_lifetime_ai_data_store();
        let mut first = crate::game_logic::GameLogic::new();
        let mut second = crate::game_logic::GameLogic::new();

        fn write_catalog(
            world: &mut crate::game_logic::GameLogic,
            max_distance: f32,
            resources: f32,
            rotate: bool,
            template: &str,
            location: (f32, f32),
        ) {
            let mut data = world.ai_definitions.baseline().clone();
            data.max_recruit_distance = max_distance;
            data.team_resources_to_build = resources;
            data.rotate_skirmish_bases = rotate;
            data.side_build_lists.clear();
            let mut list = AiSideBuildList::new("America".into());
            list.entries.push(BuildListEntry {
                template_name: template.into(),
                location,
                ..BuildListEntry::default()
            });
            data.side_build_lists.push(list);
            world.set_ai_definition_base(data);
        }
        fn assert_catalog(
            world: &crate::game_logic::GameLogic,
            max_distance: f32,
            resources: f32,
            rotate: bool,
            template: &str,
            location: (f32, f32),
        ) {
            let data = world.ai_definitions.data();
            assert_eq!(data.max_recruit_distance, max_distance);
            assert_eq!(data.team_resources_to_build, resources);
            assert_eq!(data.rotate_skirmish_bases, rotate);
            let list = data
                .side_build_lists
                .iter()
                .find(|list| list.side == "America")
                .unwrap();
            assert_eq!(list.entries.len(), 1);
            assert_eq!(list.entries[0].template_name, template);
            assert_eq!(list.entries[0].location, location);
        }

        write_catalog(
            &mut first,
            81.0,
            0.61,
            false,
            "FirstWarFactory",
            (13.0, 17.0),
        );
        write_catalog(
            &mut second,
            147.0,
            0.83,
            true,
            "SecondWarFactory",
            (23.0, 29.0),
        );

        // Construction is inert; neither newly constructed world becomes the
        // Common AI parser target; definitions stay explicitly owned.
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &process_store
        ));
        assert_catalog(&first, 81.0, 0.61, false, "FirstWarFactory", (13.0, 17.0));
        assert_catalog(&second, 147.0, 0.83, true, "SecondWarFactory", (23.0, 29.0));

        second.reset();
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &process_store
        ));
        first.reset();
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &process_store
        ));
        // Reset clears simulation state but retains both worlds' definition data.
        assert_catalog(&first, 81.0, 0.61, false, "FirstWarFactory", (13.0, 17.0));
        assert_catalog(&second, 147.0, 0.83, true, "SecondWarFactory", (23.0, 29.0));

        drop(second); // buried active entry is removed; first remains the head
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &process_store
        ));
        drop(first); // Main never changed the parser target
        assert!(Arc::ptr_eq(
            &ini_ai_data::get_ai_data_store(),
            &process_store
        ));
    }

    #[test]
    fn logic_frames_recruit_only_with_the_driving_catalog_radius() {
        if run_isolated("logic_frames_recruit_only_with_the_driving_catalog_radius") {
            return;
        }
        fn match_with_order(
            radius: f32,
        ) -> (crate::game_logic::GameLogic, crate::game_logic::ObjectId) {
            use crate::game_logic::{KindOf, Player, ThingTemplate};
            let mut world = crate::game_logic::GameLogic::new();
            world.add_player(Player::new(0, Team::China, "human", true));
            world.add_player(Player::new(1, Team::USA, "computer", false));
            let mut structure = ThingTemplate::new("CatalogOwnerStructure");
            structure.add_kind_of(KindOf::Structure);
            world.templates.insert(structure.name.clone(), structure);
            let mut infantry = ThingTemplate::new("CatalogOwnerInfantry");
            infantry.add_kind_of(KindOf::Infantry);
            world.templates.insert(infantry.name.clone(), infantry);
            world
                .create_object(
                    "CatalogOwnerStructure",
                    Team::USA,
                    Vec3::new(-100.0, 0.0, 0.0),
                )
                .unwrap();
            world
                .create_object(
                    "CatalogOwnerStructure",
                    Team::China,
                    Vec3::new(100.0, 0.0, 0.0),
                )
                .unwrap();
            let id = world
                .create_object("CatalogOwnerInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
                .unwrap();
            world.host_object_mut(id).unwrap().owner_player_id = Some(1);
            // Default-team units are deliberately recruitable beyond maxDist in C++.
            // Use a real active, recruitable source team to exercise the radius rule.
            let source = {
                let mut factory = world.team_factory.lock().unwrap();
                let mut prototype =
                    gamelogic::team::TeamPrototype::new("CatalogOwnerSource".into());
                prototype.set_ai_recruitable(true);
                prototype.set_production_priority(1);
                factory.replace_team_prototype(prototype);
                factory.create_inactive_team("CatalogOwnerSource").unwrap()
            };
            source.write().unwrap().set_active();
            world.host_object_mut(id).unwrap().team_instance_name = "CatalogOwnerSource".into();
            let mut ai = AIPlayer::new_with_team_factory(
                1,
                Team::USA,
                AIDifficulty::Medium,
                world.team_factory.clone(),
            );
            ai.base_center = Vec3::ZERO;
            ai.team_queue.push_back(AITeamQueue::new(
                "CatalogOwnerTeam".into(),
                vec![AIWorkOrder::new("CatalogOwnerInfantry".into(), 1, 100)],
                false,
                0,
            ));
            world.ai_manager.ai_players.insert(1, ai);
            let mut data = world.ai_definitions.data().clone();
            data.max_recruit_distance = radius;
            world.set_ai_definition_base(data);
            (world, id)
        }
        fn completed(world: &crate::game_logic::GameLogic) -> u32 {
            let ai = &world.ai_manager.ai_players[&1];
            ai.team_queue
                .iter()
                .chain(&ai.team_ready_queue)
                .find(|t| t.name == "CatalogOwnerTeam")
                .unwrap()
                .work_orders[0]
                .num_completed
        }
        let mut foreign = RestoreCommonStore::foreign_sentinel();
        let (mut first, first_id) = match_with_order(5.0);
        let (mut second, second_id) = match_with_order(50.0);
        assert_eq!(first_id, second_id, "both matches use the same ObjectId");
        foreign.activate();
        let dt = 1.0 / LOGIC_FRAMES_PER_SECOND;
        first.tick_logic_frame(dt, None, Some(1));
        assert_eq!(first.frame, 1);
        assert_eq!(second.frame, 0);
        assert_eq!(
            completed(&first),
            0,
            "outside this match's five-unit radius"
        );
        second.tick_logic_frame(dt, None, Some(1));
        assert_eq!(second.frame, 1);
        assert_eq!(
            completed(&second),
            1,
            "within this match's fifty-unit radius"
        );
        first.tick_logic_frame(dt, None, Some(1));
        assert_eq!(first.frame, 2);
        assert_eq!(
            completed(&first),
            0,
            "advancing another match cannot change recruitment"
        );
        assert_eq!(completed(&second), 1);
        assert_eq!(
            foreign
                .installed
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .max_recruit_distance,
            9_999.0
        );
    }

    #[test]
    fn team_affordability_and_frame_timers_use_the_driving_definitions() {
        if run_isolated("team_affordability_and_frame_timers_use_the_driving_definitions") {
            return;
        }
        let mut foreign = RestoreCommonStore::foreign_sentinel();
        let mut first = crate::game_logic::GameLogic::new();
        let mut second = crate::game_logic::GameLogic::new();
        add_world_ai(&mut first, 80.0, 0.2);
        add_world_ai(&mut second, 140.0, 0.8);
        for (world, rate) in [(&mut first, 0.5), (&mut second, 2.0)] {
            let mut unit = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
            unit.set_cost(225, 0);
            world.templates.insert(unit.name.clone(), unit);
            let mut unit = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
            unit.set_cost(700, 0);
            world.templates.insert(unit.name.clone(), unit);
            world.get_player_mut(1).unwrap().resources.supplies = 300;
            let mut data = world.ai_definitions.data().clone();
            data.resources_poor = 1_000;
            data.team_poor_mod = rate;
            world.set_ai_definition_base(data);
        }
        foreign.activate();
        let mut first_ai = first.ai_manager.ai_players.remove(&1).unwrap();
        let mut second_ai = second.ai_manager.ai_players.remove(&1).unwrap();
        assert_eq!(
            first_ai.estimate_team_unit_cost(&first, "USA_BasicForce"),
            1150
        );
        assert!(first_ai.can_afford_team_start(&first, "USA_BasicForce"));
        assert!(!second_ai.can_afford_team_start(&second, "USA_BasicForce"));
        first_ai.arm_team_timer_after_build(&first, 1.0);
        second_ai.arm_team_timer_after_build(&second, 1.0);
        // C++ frame truncation: 10*30 / poor-mod, plus the current 30th frame.
        assert_eq!(first_ai.next_team_time, 630.0 / LOGIC_FRAMES_PER_SECOND);
        assert_eq!(second_ai.next_team_time, 180.0 / LOGIC_FRAMES_PER_SECOND);
        first_ai.arm_team_timer_after_build(&first, 1.0);
        assert_eq!(first_ai.next_team_time, 630.0 / LOGIC_FRAMES_PER_SECOND);
        assert_eq!(
            foreign
                .installed
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .max_recruit_distance,
            9_999.0
        );
    }
}
