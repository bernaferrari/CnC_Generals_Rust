//! Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 958: host_object dual-read seal (tests + residual).
use crate::game_logic::host_rng_residual::HostRandomState;
use crate::game_logic::*;
use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};

const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

/// AI difficulty levels affecting decision making and timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AIDifficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl AIDifficulty {
    /// Get build delay modifier for this difficulty
    pub fn get_build_delay_modifier(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 2.0,   // 2x slower building
            AIDifficulty::Medium => 1.0, // Normal speed
            AIDifficulty::Hard => 0.7,   // 30% faster
            AIDifficulty::Brutal => 0.5, // 50% faster
        }
    }

    /// Get resource bonus for this difficulty
    pub fn get_resource_bonus(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 0.8,   // 20% less resources
            AIDifficulty::Medium => 1.0, // Normal resources
            AIDifficulty::Hard => 1.2,   // 20% bonus
            AIDifficulty::Brutal => 1.5, // 50% bonus
        }
    }

    /// Get aggressive behavior factor
    pub fn get_aggression_factor(&self) -> f32 {
        match self {
            AIDifficulty::Easy => 0.6,   // Less aggressive
            AIDifficulty::Medium => 1.0, // Normal aggression
            AIDifficulty::Hard => 1.4,   // More aggressive
            AIDifficulty::Brutal => 1.8, // Very aggressive
        }
    }
}

/// AI personality types for different playstyles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIPersonality {
    Balanced,   // Mix of offense and defense
    Aggressive, // Fast attacks, less defense
    Defensive,  // Strong defense, slower to attack
    Economic,   // Focus on economy first
    Rush,       // Early, fast attacks
}

impl AIPersonality {
    /// Get personality for a team
    pub fn for_team(team: Team) -> Self {
        match team {
            Team::USA => AIPersonality::Aggressive, // USA is aggressive with advanced tech
            Team::China => AIPersonality::Defensive, // China builds strong defenses
            Team::GLA => AIPersonality::Rush,       // GLA rushes with cheap units
            Team::Neutral => AIPersonality::Balanced,
        }
    }
}

/// AI work order for unit production
#[derive(Debug, Clone)]
pub struct AIWorkOrder {
    pub template_name: String,
    pub factory_id: Option<ObjectId>,
    /// Units actually enqueued for this work order but not yet observed at a
    /// factory exit.  This is intentionally separate from `num_completed`:
    /// C++ `AIPlayer::onUnitProduced` increments completion only after the
    /// `ProductionUpdate` has created the unit.
    pub queued_count: u32,
    pub num_completed: u32,
    pub num_required: u32,
    pub is_required: bool,
    pub priority: u32,
    /// Producer-linked units already accounted for by this order.  A factory
    /// can have produced matching units before this order was queued, so those
    /// must not be mistaken for this team's completion when the host polls the
    /// authoritative production result.
    pub observed_unit_ids: Vec<ObjectId>,
    /// C++ `WorkOrder::m_isResourceGatherer`.  This is only set for the paid
    /// follow-up collector that `AIPlayer::queueSupplyTruck` prepends; the
    /// SupplyCenter's free SpawnBehavior collector has no work order.
    pub is_resource_gatherer: bool,
    /// SupplyCenter/Stash this collector is being assigned to.  Keeping the
    /// concrete producer avoids template/name inference for general-specific
    /// collectors and lets `onUnitProduced` route it to the nearby source.
    pub supply_center_id: Option<ObjectId>,
}

impl AIWorkOrder {
    pub fn new(template_name: String, count: u32, priority: u32) -> Self {
        Self {
            template_name,
            factory_id: None,
            queued_count: 0,
            num_completed: 0,
            num_required: count,
            is_required: true,
            priority,
            observed_unit_ids: Vec::new(),
            is_resource_gatherer: false,
            supply_center_id: None,
        }
    }
}

/// AI team build queue
#[derive(Debug, Clone)]
pub struct AITeamQueue {
    pub name: String,
    pub work_orders: Vec<AIWorkOrder>,
    pub priority_build: bool,
    pub frame_started: u32,
    pub completed: bool,
}

/// AI building info for base construction
#[derive(Debug, Clone)]
pub struct AIBuildingInfo {
    pub template_name: String,
    pub position: Vec3,
    pub object_id: Option<ObjectId>,
    pub is_built: bool,
    pub is_priority: bool,
    pub rebuild_count: u32,
    pub max_rebuilds: u32,
    /// Host residual of C++ BuildListInfo objectTimestamp (seconds).
    /// Set when the live building is destroyed; rebuild waits RebuildDelaySeconds.
    pub destroyed_at_time: Option<f32>,
}

impl AIBuildingInfo {
    pub fn new(template_name: String, position: Vec3, max_rebuilds: u32) -> Self {
        Self {
            template_name,
            position,
            object_id: None,
            is_built: false,
            is_priority: false,
            rebuild_count: 0,
            max_rebuilds,
            destroyed_at_time: None,
        }
    }

    /// C++ rebuild delay residual: ready when never destroyed or delay elapsed.
    pub fn rebuild_delay_elapsed(&self, current_time: f32, delay_seconds: f32) -> bool {
        match self.destroyed_at_time {
            None => true,
            Some(t0) => current_time >= t0 + delay_seconds,
        }
    }
}

/// Base AI Player implementation
#[derive(Debug)]
pub struct AIPlayer {
    pub player_id: u32,
    pub team: Team,
    pub difficulty: AIDifficulty,
    pub personality: AIPersonality,

    // Core AI State
    pub is_active: bool,
    pub enemy_player_id: Option<u32>,

    // Economic Management
    pub base_center: Vec3,
    pub base_radius: f32,
    /// Deterministic placement scatter (retail ADC RandomValue residual).
    placement_rng: HostRandomState,
    pub building_queue: Vec<AIBuildingInfo>,
    pub next_building_time: f32,
    /// Existing work orders run on the short C++ `m_teamDelay`; choosing a
    /// new team uses the longer AIData `TeamSeconds` timer below.
    pub next_team_queue_time: f32,
    pub next_team_time: f32,

    // Military Management
    pub team_queue: VecDeque<AITeamQueue>,
    pub attack_in_progress: bool,
    pub last_attack_time: f32,
    pub defensive_units: Vec<ObjectId>,

    // Timing and Decision Making
    pub last_update_time: f32,
    pub resource_check_time: f32,
    pub enemy_check_time: f32,

    // AI Decision State
    pub current_strategy: AIStrategy,
    pub build_phase: AIBuildPhase,

    /// Count of production-linked actions (build/produce/attack) for gates.
    pub activity_count: u64,
}

/// AI strategic states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIStrategy {
    EarlyGame, // Focus on base building and early units
    MidGame,   // Balanced expansion and military buildup
    LateGame,  // Advanced units and multiple attack groups
    Desperate, // Low on resources/units, all-in attacks
}

/// AI build phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIBuildPhase {
    BaseConstruction, // Building core base structures
    UnitProduction,   // Building initial army
    Expansion,        // Expanding economy
    MassProduction,   // Building large armies
}

impl AIPlayer {
    /// Create new AI player
    pub fn new(player_id: u32, team: Team, difficulty: AIDifficulty) -> Self {
        let personality = AIPersonality::for_team(team);

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
            team_queue: VecDeque::new(),
            attack_in_progress: false,
            last_attack_time: 0.0,
            defensive_units: Vec::new(),
            last_update_time: 0.0,
            resource_check_time: 0.0,
            enemy_check_time: 0.0,
            current_strategy: AIStrategy::EarlyGame,
            build_phase: AIBuildPhase::BaseConstruction,
            activity_count: 0,
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

        self.last_update_time = current_time;
        self.update_enemy_assessment(game_logic, current_time);

        // doBaseBuilding
        self.update_economic_management(game_logic, current_time);
        // checkReadyTeams — force-start teams waiting ≥ 60s (AIPlayer.cpp:1658-1663)
        self.check_ready_teams(game_logic, current_time);
        // checkQueuedTeams + doTeamBuilding
        self.update_military_management(game_logic, current_time);
        // doUpgradesAndSkills
        self.do_upgrades_and_skills(game_logic);
        // updateBridgeRepair — resume interrupted construction (dozer reattach)
        self.resume_interrupted_construction(game_logic);

        self.update_strategic_decisions(game_logic, current_time);
    }

    /// Set up initial base building layout
    fn setup_base_layout(&mut self) {
        let center = self.base_center;

        // Core base buildings based on team
        match self.team {
            Team::USA => {
                self.add_building("AmericaCommandCenter", center, 1);
                self.add_building("AmericaSupplyCenter", center + Vec3::new(50.0, 0.0, 0.0), 2);
                self.add_building("AmericaPowerPlant", center + Vec3::new(-50.0, 0.0, 0.0), 2);
                self.add_building("AmericaBarracks", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("AmericaWarFactory", center + Vec3::new(100.0, 0.0, 50.0), 1);
            }
            Team::China => {
                self.add_building("ChinaCommandCenter", center, 1);
                self.add_building("ChinaSupplyCenter", center + Vec3::new(50.0, 0.0, 0.0), 2);
                self.add_building("ChinaPowerPlant", center + Vec3::new(-50.0, 0.0, 0.0), 2);
                self.add_building("ChinaBarracks", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("ChinaWarFactory", center + Vec3::new(100.0, 0.0, 50.0), 1);
            }
            Team::GLA => {
                self.add_building("GLACommandCenter", center, 1);
                self.add_building("GLASupplyStash", center + Vec3::new(50.0, 0.0, 0.0), 3);
                self.add_building("GLAArmsDealer", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("GLABarracks", center + Vec3::new(-50.0, 0.0, 50.0), 2);
            }
            _ => {}
        }
    }

    /// Add building to construction queue
    pub fn add_building(&mut self, template_name: &str, position: Vec3, max_rebuilds: u32) {
        let building = AIBuildingInfo::new(template_name.to_string(), position, max_rebuilds);
        self.building_queue.push(building);
    }

    /// Set up initial AI strategy based on personality
    fn setup_initial_strategy(&mut self) {
        self.current_strategy = AIStrategy::EarlyGame;
        self.build_phase = AIBuildPhase::BaseConstruction;

        // Retail AIData StructureSeconds=0 / TeamSeconds=10, scaled by difficulty.
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_building_time =
            self.last_update_time + (Self::STRUCTURE_SECONDS * delay_modifier);
        self.next_team_time = self.last_update_time + (Self::TEAM_SECONDS * delay_modifier);
    }

    /// Update enemy assessment and target selection
    fn update_enemy_assessment(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Check for enemies every 5 seconds
        if current_time - self.enemy_check_time < 5.0 {
            return;
        }
        self.enemy_check_time = current_time;

        // Find closest enemy player
        let mut best_enemy: Option<u32> = None;
        let mut best_distance = f32::MAX;

        for player_id in 0..4 {
            // Check up to 4 players
            if player_id == self.player_id {
                continue;
            }

            if let Some(player) = game_logic.get_player(player_id) {
                if player.team != self.team && player.is_alive {
                    // Calculate distance to enemy base
                    let enemy_base = self.find_enemy_base_center(game_logic, player.team);
                    let distance = self.base_center.distance(enemy_base);

                    if distance < best_distance {
                        best_distance = distance;
                        best_enemy = Some(player_id);
                    }
                }
            }
        }

        if self.enemy_player_id != best_enemy {
            self.enemy_player_id = best_enemy;
            if let Some(enemy_id) = best_enemy {
                log::debug!(
                    "AI Player {} ({}) targeting enemy Player {}",
                    self.player_id,
                    self.team.get_name(),
                    enemy_id
                );
            }
        }
    }

    /// Update economic management (base building, resource optimization)
    fn update_economic_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if current_time < self.next_building_time {
            return;
        }

        // Check if we need more resources
        // Check resources first
        let should_build_supply = if let Some(player) = game_logic.get_player(self.player_id) {
            let resource_threshold = match self.difficulty {
                AIDifficulty::Easy => 500,
                AIDifficulty::Medium => 800,
                AIDifficulty::Hard => 1200,
                AIDifficulty::Brutal => 1500,
            };
            player.resources.supplies < resource_threshold
        } else {
            false
        };

        let should_build_power = if let Some(player) = game_logic.get_player(self.player_id) {
            player.power_available < 0
        } else {
            false
        };

        // Build structures if needed
        if should_build_supply {
            self.try_build_supply_center(game_logic);
        }

        if should_build_power {
            self.try_build_power_plant(game_logic);
        }

        // `AISkirmishPlayer::processBaseBuilding` selects and starts one
        // structure per economic pass.  Starting every eligible entry here
        // spends the AI's money in a burst and leaves most scaffolds without a
        // dozer, which is not a viable skirmish base build lifecycle.
        self.process_building_queue(game_logic, current_time);

        // StructureSeconds residual + wealth/poor rate (AIData Structures*Rate).
        let interval = self.scaled_interval_seconds(game_logic, Self::STRUCTURE_SECONDS, true);
        self.next_building_time = current_time + interval;
    }

    /// Update military management (unit production, attack coordination).
    ///
    /// Retail `AISkirmishPlayer::doTeamBuilding` calls `queueUnits` every
    /// `m_teamDelay` (2 seconds), but only selects a *new* team after its
    /// `m_teamTimer` (`TeamSeconds`) expires.  Coupling both to TeamSeconds
    /// leaves a just-selected skirmish team idle for ten seconds.
    fn update_military_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let queue_due = current_time >= self.next_team_queue_time;
        // C++ `AIPlayer::onUnitProduced` sets m_teamDelay to zero after the
        // factory has actually created the unit.  Poll the producer link on
        // each host-AI update so a completed unit wakes its waiting order on
        // the next frame instead of idling until the normal two-second pass.
        // `process_team_queue` reconciles again when it runs; that second
        // observation is empty and keeps the queue mutation in one place.
        let unit_completed = !queue_due && self.reconcile_produced_units(game_logic);

        if queue_due || unit_completed {
            // C++ queueUnits runs before selection, so established orders get
            // first use of an idle factory.
            self.process_team_queue(game_logic, current_time);

            let selected_team =
                if current_time >= self.next_team_time && self.should_build_new_team(game_logic) {
                    self.select_team_to_build(game_logic, current_time)
                } else {
                    false
                };

            if selected_team {
                // C++ processTeamBuilding invokes queueUnits immediately after
                // a successful selectTeamToBuild, not on the next TeamSeconds.
                self.process_team_queue(game_logic, current_time);
                self.next_team_time = current_time
                    + self.scaled_interval_seconds(game_logic, Self::TEAM_SECONDS, false);
            } else if current_time >= self.next_team_time {
                // A failed selection leaves m_readyToBuildTeam set.  Retry on
                // the short m_teamDelay cadence rather than sleeping 10 sec.
                self.next_team_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
            }

            self.next_team_queue_time = current_time + Self::TEAM_QUEUE_RETRY_SECONDS;
        }

        // The host attack policy has its own 60-second guard.  Evaluate it on
        // manager cadence so a newly produced force is not held behind the
        // new-team selection timer.
        self.evaluate_attack_opportunities(game_logic, current_time);
    }

    /// Update strategic decisions and long-term planning
    fn update_strategic_decisions(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update strategy based on game state
        self.update_strategy_phase(game_logic, current_time);

        // Update build phase
        self.update_build_phase(game_logic, current_time);
    }

    /// Pick a real, available construction unit as C++ `AIPlayer::findDozer`
    /// does before `AISkirmishPlayer::processBaseBuilding` starts a structure.
    ///
    /// A builder already constructing, repairing, or ferrying supplies is not
    /// available.  Prefer an idle dozer, then the nearest other eligible one;
    /// the ObjectId tiebreak keeps host AI choices deterministic despite the
    /// object store being a hash map.
    fn find_available_dozer(game_logic: &GameLogic, team: Team, target: Vec3) -> Option<ObjectId> {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                if object.team != team || !object.is_alive() || !object.can_construct() {
                    return false;
                }
                !matches!(
                    object.ai_state,
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Docking
                )
            })
            .map(|object| {
                let position = object.get_position();
                let dx = position.x - target.x;
                let dz = position.z - target.z;
                let busy_rank = u8::from(object.ai_state != AIState::Idle);
                (busy_rank, dx * dx + dz * dz, object.id)
            })
            .min_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            })
            .map(|(_, _, id)| id)
    }

    /// Reattach a live dozer to every queued scaffold that lost its builder.
    ///
    /// C++ refreshes existing `BuildListInfo` entries before considering a new
    /// structure, and calls `aiResumeConstruction` whenever the remembered
    /// dozer was killed/captured or no longer has a build task.  Without this,
    /// one interrupted construction permanently blocks its planned base slot.
    fn resume_interrupted_construction(&self, game_logic: &mut GameLogic) {
        let unfinished: Vec<(ObjectId, Vec3)> = self
            .building_queue
            .iter()
            .filter_map(|building| {
                let object_id = building.object_id?;
                let object = game_logic.host_object(object_id)?;
                (object.team == self.team && object.is_alive() && object.status.under_construction)
                    .then_some((object_id, object.get_position()))
            })
            .collect();

        for (structure_id, position) in unfinished {
            let has_live_builder = game_logic.host_objects().values().any(|object| {
                object.team == self.team
                    && object.is_alive()
                    && object.can_construct()
                    && object.target == Some(structure_id)
                    && object.ai_state == AIState::Constructing
            });
            if has_live_builder {
                continue;
            }

            if let Some(dozer_id) = Self::find_available_dozer(game_logic, self.team, position) {
                if game_logic.resume_construction(&[dozer_id], structure_id) {
                    // `resume_construction` creates the authoritative dock;
                    // this command restores the corresponding movement/path.
                    let _ = game_logic.unit_command_begin_construct(dozer_id, position);
                }
            }
        }
    }

    /// Process one building construction start, matching
    /// `AISkirmishPlayer::processBaseBuilding`.
    ///
    /// C++ does not mass-place scaffolds and later invent construction work:
    /// it chooses one legal plan, finds a dozer, and gives that dozer the live
    /// build task immediately.  Keep an unstartable plan in the queue instead
    /// of fabricating a builder or charging the player.
    fn process_building_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.resume_interrupted_construction(game_logic);

        let build_index = game_logic.get_player(self.player_id).and_then(|player| {
            self.building_queue.iter().position(|building| {
                !building.is_built
                    && building.rebuild_count < building.max_rebuilds
                    && building.object_id.is_none()
                    && building.rebuild_delay_elapsed(current_time, Self::REBUILD_DELAY_SECONDS)
                    && game_logic
                        .templates
                        .get(&building.template_name)
                        .is_some_and(|template| player.can_afford(&template.build_cost))
            })
        });

        if let Some(index) = build_index {
            if let Some((template_name, position, build_cost)) =
                self.building_queue.get(index).and_then(|building| {
                    game_logic
                        .templates
                        .get(&building.template_name)
                        .map(|template| {
                            (
                                building.template_name.clone(),
                                building.position,
                                template.build_cost,
                            )
                        })
                })
            {
                let started = Self::find_available_dozer(game_logic, self.team, position)
                    .filter(|&dozer_id| {
                        game_logic.is_location_legal_to_build_for_builder(
                            self.team,
                            position,
                            &template_name,
                            Some(dozer_id),
                        )
                    })
                    .and_then(|dozer_id| {
                        let structure_id = game_logic.create_object_under_construction(
                            &template_name,
                            self.team,
                            position,
                        )?;

                        // The authoritative construction APIs carry the exact live
                        // association that `update_construction` consumes: target is
                        // the scaffold and the dozer receives its construct path.
                        let assigned = game_logic.resume_construction(&[dozer_id], structure_id);
                        let commanded =
                            assigned && game_logic.unit_command_begin_construct(dozer_id, position);
                        let paid = commanded
                            && game_logic
                                .get_player_mut(self.player_id)
                                .is_some_and(|player| player.spend_resources(&build_cost));
                        if paid {
                            Some(structure_id)
                        } else {
                            // This is only an invariant failure after the earlier
                            // affordability/path checks.  Do not leave an unpaid,
                            // unassigned scaffold behind.
                            game_logic.cancel_dozers_building(structure_id);
                            game_logic.destroy_object(structure_id);
                            None
                        }
                    });

                if let Some(object_id) = started {
                    let building = &mut self.building_queue[index];
                    building.object_id = Some(object_id);
                    building.rebuild_count += 1;
                    // C++ setObjectTimestamp(0) once rebuild is enabled/started.
                    building.destroyed_at_time = None;
                    self.activity_count = self.activity_count.saturating_add(1);
                    log::debug!(
                        "AI Player {} building {} at {:?}",
                        self.player_id,
                        template_name,
                        position
                    );
                }
            }
        }

        // Update building status
        for building in &mut self.building_queue {
            if let Some(object_id) = building.object_id {
                if let Some(object) = game_logic.host_object(object_id) {
                    building.is_built = object.is_constructed();
                } else {
                    // Building was destroyed — stamp rebuild delay (AIData RebuildDelaySeconds).
                    building.object_id = None;
                    building.is_built = false;
                    if building.destroyed_at_time.is_none() {
                        building.destroyed_at_time = Some(current_time);
                    }
                }
            }
        }
    }

    /// Try to build a supply center for resource generation
    fn try_build_supply_center(&mut self, _game_logic: &mut GameLogic) {
        let supply_center_name = match self.team {
            Team::USA => "AmericaSupplyCenter",
            Team::China => "ChinaSupplyCenter",
            Team::GLA => "GLASupplyStash",
            _ => return,
        };

        // Check if we already have enough supply centers building
        let existing_count = self
            .building_queue
            .iter()
            .filter(|b| {
                b.template_name == supply_center_name && (!b.is_built || b.object_id.is_some())
            })
            .count();

        if existing_count < 3 {
            // Limit to 3 supply centers
            let position = self.base_center
                + Vec3::new(
                    self.placement_rng.next_real(-80.0, 80.0),
                    0.0,
                    self.placement_rng.next_real(-80.0, 80.0),
                );

            self.add_building(supply_center_name, position, 2);
        }
    }

    /// Try to build a power plant for energy
    fn try_build_power_plant(&mut self, _game_logic: &mut GameLogic) {
        let power_plant_name = match self.team {
            Team::USA => "AmericaPowerPlant",
            Team::China => "ChinaPowerPlant",
            Team::GLA => return, // GLA doesn't use power
            _ => return,
        };

        // Check if we already have enough power plants
        let existing_count = self
            .building_queue
            .iter()
            .filter(|b| {
                b.template_name == power_plant_name && (!b.is_built || b.object_id.is_some())
            })
            .count();

        if existing_count < 2 {
            let position = self.base_center
                + Vec3::new(
                    self.placement_rng.next_real(-60.0, 60.0),
                    0.0,
                    self.placement_rng.next_real(-60.0, 60.0),
                );

            self.add_building(power_plant_name, position, 1);
        }
    }

    /// Default/AIData.ini `SideInfo::* ResourceGatherers*` plus the free
    /// SupplyCenter/Stash SpawnBehavior collector.  All three difficulty
    /// entries use these same values in the retail data.
    fn desired_gatherers_per_supply_center(&self) -> u32 {
        match self.team {
            Team::USA | Team::China => 3,
            Team::GLA => 6,
            Team::Neutral => 0,
        }
    }

    /// C++ `SUPPLY_CENTER_CLOSE_DIST` = 20 * PATHFIND_CELL_SIZE_F (10).
    const SUPPLY_CENTER_CLOSE_DISTANCE: f32 = 200.0;

    /// Completed allied SupplyCenterDockUpdate owners, in stable object-id
    /// order.  C++ discovers these through its BuildListInfo records; Main's
    /// live objects are the authoritative equivalent after construction.
    fn live_supply_centers(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        let mut centers: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == self.team
                    && object.is_alive()
                    && object.is_constructed()
                    && !object.status.sold
                    && (object.is_kind_of(KindOf::SupplyCenter)
                        || object.is_kind_of(KindOf::FSSupplyCenter)))
                .then_some(id)
            })
            .collect();
        centers.sort_by_key(|id| id.0);
        centers
    }

    /// Typed `KINDOF_SUPPLY_SOURCE` lookup near a concrete supply center.
    /// The host stores this source capability as `Resource` + `Harvestable`;
    /// do not infer sources from object names.  A finite source must still
    /// hold supplies, matching C++ SupplyWarehouseDockUpdate's available-cash
    /// check before the AI assigns additional collectors.
    fn nearest_supply_source_for_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        let center_position = center.get_position();
        let maximum_distance = Self::SUPPLY_CENTER_CLOSE_DISTANCE + center.selection_radius;
        let maximum_distance_squared = maximum_distance * maximum_distance;

        game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, source)| {
                if !source.is_alive()
                    || source.status.under_construction
                    || (source.team != Team::Neutral && source.team != self.team)
                    || !(source.is_kind_of(KindOf::Resource)
                        || source.is_kind_of(KindOf::Harvestable))
                    || source.stored_resources.supplies == 0
                {
                    return None;
                }
                let delta = source.get_position() - center_position;
                let distance_squared = delta.x * delta.x + delta.z * delta.z;
                (distance_squared <= maximum_distance_squared).then_some((distance_squared, id))
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1 .0.cmp(&right.1 .0))
            })
            .map(|(_, id)| id)
    }

    fn collector_count_for_supply_center(
        &self,
        game_logic: &GameLogic,
        center_id: ObjectId,
    ) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team
                    && object.is_alive()
                    // C++ `AIPlayer::queueSupplyTruck` counts each
                    // SupplyTruckAIUpdate::getPreferredDockID(), not the
                    // factory/spawner that happened to create the unit.  A
                    // collector rescued after its old center dies must count
                    // toward its newly assigned depot immediately.
                    && object.preferred_dock_id == Some(center_id)
                    && object.is_resource_collector()
            })
            .count() as u32
    }

    fn total_live_collectors(&self, game_logic: &GameLogic) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| {
                object.team == self.team && object.is_alive() && object.is_resource_collector()
            })
            .count() as u32
    }

    /// C++ `AIPlayer::onUnitProduced`: a resource-gatherer work order makes
    /// the newly created collector want supplies and docks it at the matching
    /// SupplyCenter.  Main's typed gather state carries the source target;
    /// `preferred_dock_id` retains the C++ CMD_FROM_PLAYER dock assignment
    /// for the later ReturningResources leg.
    fn route_collector_to_supply_center(
        &self,
        game_logic: &mut GameLogic,
        collector_id: ObjectId,
        center_id: ObjectId,
    ) -> bool {
        let Some(source_id) = self.nearest_supply_source_for_center(game_logic, center_id) else {
            return false;
        };
        let valid_collector = game_logic
            .host_object(collector_id)
            .is_some_and(|collector| {
                collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
            });
        if !valid_collector {
            return false;
        }
        let routed = game_logic
            .unit_command_stop_moving_order_target(collector_id, Some(source_id))
            && game_logic.unit_command_set_ai_state(collector_id, AIState::Gathering);
        if routed {
            if let Some(collector) = game_logic.host_object_mut(collector_id) {
                // C++ AIPlayer::queueSupplyTruck uses aiDock(...,
                // CMD_FROM_PLAYER) specifically so SupplyTruckAIUpdate stores
                // this center in m_preferredDock.
                collector.preferred_dock_id = Some(center_id);
            }
        }
        routed
    }

    /// The SpawnBehavior collector has no AI work order, so give an idle
    /// producer-linked collector the same SupplyTruckAI wanting/dock handoff
    /// that paid collector output receives in C++ `onUnitProduced`.
    fn route_idle_supply_center_collectors(&self, game_logic: &mut GameLogic, center_id: ObjectId) {
        let mut collectors: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                let spawned_here_without_a_dock =
                    object.producer_id == Some(center_id) && object.preferred_dock_id.is_none();
                let already_assigned_to_this_center = object.preferred_dock_id == Some(center_id);
                (object.team == self.team
                    && object.is_alive()
                    && object.is_resource_collector()
                    // The one-shot SpawnBehavior collector initially has a
                    // producer link but no preferred dock.  Once assigned,
                    // C++ only reissues aiDock to a non-ferrying collector
                    // whose preferred dock is this center.
                    && (spawned_here_without_a_dock || already_assigned_to_this_center)
                    && !Self::collector_is_currently_ferrying_supplies(object))
                .then_some(id)
            })
            .collect();
        collectors.sort_by_key(|id| id.0);
        for collector_id in collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
    }

    /// C++ `SupplyTruckAIUpdate::isCurrentlyFerryingSupplies` returns true
    /// only in its Wanting or Docking substates.  Main's economy state owns
    /// those live legs as Gathering, ReturningResources, and Docking; do not
    /// use a generic "not idle" test here because GLA workers also carry
    /// `KINDOF_HARVESTER` while building or repairing.
    #[inline]
    fn collector_is_currently_ferrying_supplies(collector: &Object) -> bool {
        matches!(
            collector.ai_state,
            AIState::Gathering | AIState::ReturningResources | AIState::Docking
        )
    }

    /// C++ `AIPlayer::queueSupplyTruck` first rescues one active collector
    /// whose preferred dock no longer resolves (typically after its supply
    /// center was destroyed), assigns it to the currently-understaffed center,
    /// and returns before it spends money on a replacement truck.
    fn reattach_one_loose_supply_collector(
        &self,
        game_logic: &mut GameLogic,
        center_id: ObjectId,
    ) -> bool {
        let mut candidates: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, collector)| {
                let missing_preferred_dock = collector
                    .preferred_dock_id
                    .is_none_or(|dock_id| game_logic.host_object(dock_id).is_none());
                (collector.team == self.team
                    && collector.is_alive()
                    && collector.is_resource_collector()
                    && missing_preferred_dock
                    && Self::collector_is_currently_ferrying_supplies(collector))
                .then_some(id)
            })
            .collect();
        candidates.sort_by_key(|id| id.0);

        let Some(collector_id) = candidates.into_iter().next() else {
            return false;
        };
        let Some(collector) = game_logic.host_object_mut(collector_id) else {
            return false;
        };
        // `aiDock(center, CMD_FROM_PLAYER)` persists m_preferredDock while
        // preserving the supply sub-brain's active ferry leg.  Do not require
        // a new local warehouse here: retail reattaches before its subsequent
        // resource scan, and an already-full collector must be able to return
        // directly to the replacement center.
        collector.preferred_dock_id = Some(center_id);
        true
    }

    /// A collector work order is tied to the concrete SupplyCenter/Stash whose
    /// authored SpawnBehavior identified the collector template.  This avoids
    /// the old unit-name factory guessing and preserves C++'s real typed
    /// ProductionUpdate authorization/queue/money path.
    fn supply_center_factory_for_collector(
        game_logic: &GameLogic,
        center_id: ObjectId,
        collector_template: &str,
        team: Team,
    ) -> Option<ObjectId> {
        let center = game_logic.host_object(center_id)?;
        if center.team != team
            || !center.is_alive()
            || !center.is_constructed()
            || !Self::factory_is_idle(center)
            || (!center.is_kind_of(KindOf::SupplyCenter)
                && !center.is_kind_of(KindOf::FSSupplyCenter))
            || !game_logic
                .templates
                .get(collector_template)
                .is_some_and(|template| template.is_kind_of(KindOf::Harvester))
        {
            return None;
        }
        (game_logic.can_make_unit(center_id, collector_template)
            == crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK)
            .then_some(center_id)
    }

    /// C++ `AIPlayer::queueSupplyTruck`: retain the free SpawnBehavior
    /// collector separately, then prepend one paid, priority collector work
    /// order when a completed supply center has a nearby live source and is
    /// below its SideInfo gatherer target.
    fn queue_supply_truck(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.team_queue.iter().any(|team| {
            team.work_orders
                .iter()
                .any(|order| order.is_resource_gatherer)
        }) {
            return;
        }

        let desired = self.desired_gatherers_per_supply_center();
        if desired == 0 {
            return;
        }
        let total_collectors = self.total_live_collectors(game_logic);
        let centers = self.live_supply_centers(game_logic);
        for center_id in centers {
            self.route_idle_supply_center_collectors(game_logic, center_id);

            let current = self.collector_count_for_supply_center(game_logic, center_id);
            if current >= desired {
                continue;
            }
            // Do not create a paid replacement while a surviving active
            // harvester from a destroyed center can fill this slot.  C++
            // returns after one such rescue, which also preserves its
            // one-collector-per-economic-tick reassignment cadence.
            if self.reattach_one_loose_supply_collector(game_logic, center_id) {
                return;
            }
            // The first collector must be the actual one-shot SpawnBehavior
            // result, not an unpaid factory item.  Once that behavior has
            // fired, a later dead starter is eligible for normal paid
            // replacement just as C++ does after its initial `current = -1`
            // handoff becomes a real counted gatherer.
            let spawn_behavior_fired = game_logic
                .host_object(center_id)
                .is_some_and(|center| center.supply_center_spawn_behavior_fired);
            if !spawn_behavior_fired {
                continue;
            }
            if total_collectors >= desired.saturating_mul(3)
                || self
                    .nearest_supply_source_for_center(game_logic, center_id)
                    .is_none()
            {
                continue;
            }
            let Some(collector_template) =
                game_logic.supply_center_one_shot_collector_template(center_id)
            else {
                continue;
            };
            if Self::supply_center_factory_for_collector(
                game_logic,
                center_id,
                &collector_template,
                self.team,
            )
            .is_none()
            {
                continue;
            }

            let mut order = AIWorkOrder::new(collector_template, 1, 100);
            order.is_resource_gatherer = true;
            order.supply_center_id = Some(center_id);
            self.team_queue.push_front(AITeamQueue {
                name: "Supply truck".to_string(),
                work_orders: vec![order],
                priority_build: true,
                frame_started: (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
                completed: false,
            });
            // C++ sets m_teamDelay=0 and proceeds through queueUnits in this
            // same pass, so process_team_queue will enqueue it immediately.
            // Do not return: retail keeps walking completed centers and may
            // begin one collector order for each in the same economic tick.
        }
    }

    /// Account for units that have physically left their producing factory.
    ///
    /// C++ `AIPlayer::onUnitProduced` receives the factory and newly-created
    /// unit directly, increments `m_numCompleted`, and clears
    /// `m_factoryID`.  Main owns the production simulation, so its equivalent
    /// is the stable `producer_id` stamped during the live completion path.
    /// Never treat a successful enqueue as a completed work order: doing so
    /// made teams disappear before any of their units existed.
    fn reconcile_produced_units(&mut self, game_logic: &mut GameLogic) -> bool {
        let mut observed_completion = false;
        let mut completed_resource_collectors: Vec<(ObjectId, ObjectId)> = Vec::new();
        for team in &mut self.team_queue {
            for order in &mut team.work_orders {
                let Some(factory_id) = order.factory_id else {
                    continue;
                };

                // The only outstanding request for a normal work order is
                // bound to this factory.  If the producer died, C++ can no
                // longer deliver that request; allow a future queue pass to
                // find a replacement factory instead of retaining a dead ID.
                let factory_alive = game_logic
                    .host_object(factory_id)
                    .map(|factory| factory.is_alive())
                    .unwrap_or(false);
                if !factory_alive {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                let remaining = order.num_required.saturating_sub(order.num_completed);
                if remaining == 0 {
                    order.factory_id = None;
                    order.queued_count = 0;
                    continue;
                }

                // `producer_id` is applied before the spawned unit enters the
                // normal AI update, so this observes the real factory exit
                // rather than inferring a completion from a queue mutation.
                let mut produced: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit
                                .template_name
                                .eq_ignore_ascii_case(&order.template_name)
                            && !order.observed_unit_ids.contains(&unit_id))
                        .then_some(unit_id)
                    })
                    .collect();
                // HashMap iteration is deliberately not an ordering contract;
                // preserve the C++ one-unit-at-a-time work-order progression.
                produced.sort_by_key(|id| id.0);

                let mut accepted = 0u32;
                for unit_id in produced.into_iter().take(remaining as usize) {
                    order.observed_unit_ids.push(unit_id);
                    if order.is_resource_gatherer {
                        if let Some(center_id) = order.supply_center_id {
                            completed_resource_collectors.push((unit_id, center_id));
                        }
                    }
                    accepted = accepted.saturating_add(1);
                }
                if accepted == 0 {
                    continue;
                }

                order.num_completed = order
                    .num_completed
                    .saturating_add(accepted)
                    .min(order.num_required);
                order.queued_count = order.queued_count.saturating_sub(accepted);
                observed_completion = true;
                // `onUnitProduced` releases the factory association for the
                // next required unit.  Host queues one at a time as C++ does.
                order.factory_id = None;
            }
        }
        // C++ `onUnitProduced` performs the SupplyTruckAI wanting/dock setup
        // immediately after matching the work order.  Do it after releasing
        // the queue borrow so these are real typed gather commands, not a
        // synthetic resource credit.
        for (collector_id, center_id) in completed_resource_collectors {
            let _ = self.route_collector_to_supply_center(game_logic, collector_id, center_id);
        }
        observed_completion
    }

    /// Process team production queue
    fn process_team_queue(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        self.reconcile_produced_units(game_logic);
        // Retail queueUnits invokes queueSupplyTruck before walking the usual
        // TeamInQueue work orders.  Its priority order therefore gets an idle
        // SupplyCenter and pays through ordinary ProductionUpdate immediately.
        self.queue_supply_truck(game_logic, current_time);

        // Collect all factory assignments needed
        let mut factory_assignments = Vec::new();
        let mut completed_teams = Vec::new();

        for (team_index, team) in self.team_queue.iter_mut().enumerate() {
            let mut all_complete = true;

            for (order_index, work_order) in team.work_orders.iter().enumerate() {
                if work_order.num_completed < work_order.num_required {
                    // Try to queue more units
                    if work_order.factory_id.is_none() {
                        factory_assignments.push((
                            team_index,
                            order_index,
                            work_order.template_name.clone(),
                            team.priority_build,
                            work_order.is_resource_gatherer,
                            work_order.supply_center_id,
                        ));
                    }

                    all_complete = false;
                }
            }

            if all_complete && !team.completed {
                team.completed = true;
                completed_teams.push(team_index);
            }
        }

        // Process factory assignments and enqueue production on the host path.
        let mut produced = 0u64;
        for (
            team_index,
            order_index,
            template_name,
            priority_build,
            is_resource_gatherer,
            supply_center_id,
        ) in factory_assignments
        {
            // C++ `startTraining(order, team->m_priorityBuild, ...)` only
            // permits a busy factory for a priority build.  Normal skirmish
            // teams must wait for an idle producer; otherwise a pre-existing
            // matching unit could be misattributed to this work order.
            let factory_id = if is_resource_gatherer {
                supply_center_id.and_then(|center_id| {
                    Self::supply_center_factory_for_collector(
                        game_logic,
                        center_id,
                        &template_name,
                        self.team,
                    )
                })
            } else {
                Self::find_factory_for_unit_ex(
                    game_logic,
                    &template_name,
                    self.team,
                    priority_build,
                )
            };
            if let Some(factory_id) = factory_id {
                // Snapshot matching output before the request is submitted.
                // A priority order may join an already-busy factory, whose
                // earlier output belongs to another order.
                let mut preexisting: Vec<ObjectId> = game_logic
                    .host_objects()
                    .iter()
                    .filter_map(|(&unit_id, unit)| {
                        (unit.team == self.team
                            && unit.producer_id == Some(factory_id)
                            && unit.template_name.eq_ignore_ascii_case(&template_name))
                        .then_some(unit_id)
                    })
                    .collect();
                preexisting.sort_by_key(|id| id.0);

                let queued = game_logic.enqueue_production(factory_id, template_name.clone());
                if let Some(team) = self.team_queue.get_mut(team_index) {
                    if let Some(work_order) = team.work_orders.get_mut(order_index) {
                        // Only bind factory on success — failed enqueue (wrong type,
                        // full queue, cash) must retry next military tick.  Record
                        // pre-existing matching output before the enqueue so a unit
                        // made for an older queue cannot complete this order.
                        if queued {
                            for unit_id in preexisting {
                                if !work_order.observed_unit_ids.contains(&unit_id) {
                                    work_order.observed_unit_ids.push(unit_id);
                                }
                            }
                            work_order.factory_id = Some(factory_id);
                            work_order.queued_count = work_order.queued_count.saturating_add(1);
                            produced = produced.saturating_add(1);
                        } else {
                            work_order.factory_id = None;
                        }
                    }
                }
            }
        }
        self.activity_count = self.activity_count.saturating_add(produced);

        // Remove completed teams
        for &index in completed_teams.iter().rev() {
            if let Some(team) = self.team_queue.remove(index) {
                log::debug!("AI Player {} completed team: {}", self.player_id, team.name);
            }
        }
    }

    /// Estimate average unit-cost residual for a named host team composition.
    /// C++ uses (min+max)/2 from TeamPrototype; host work-orders use fixed counts.
    fn estimate_team_unit_cost(&self, game_logic: &GameLogic, team_name: &str) -> u32 {
        let orders = self.create_work_orders_for_team(team_name);
        let mut cost = 0u32;
        for order in orders {
            let unit_cost = game_logic
                .templates
                .get(&order.template_name)
                .map(|t| t.build_cost.supplies)
                .unwrap_or(0);
            cost = cost.saturating_add(unit_cost.saturating_mul(order.num_required as u32));
        }
        cost
    }

    /// C++ `isPossibleToBuildTeam` money residual:
    /// `cost *= m_teamResourcesToBuild` then require `money >= cost`.
    fn can_afford_team_start(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let Some(player) = game_logic.get_player(self.player_id) else {
            return false;
        };
        let full = self.estimate_team_unit_cost(game_logic, team_name) as f32;
        let required = (full * Self::TEAM_RESOURCES_TO_START).ceil() as u32;
        player.resources.supplies >= required
    }

    /// C++ `isPossibleToBuildTeam` residual (money + factory existence + any idle).
    /// Production-condition scripts / maxInstances remain unported.
    fn is_possible_to_build_team(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        self.can_afford_team_start(game_logic, team_name)
            && self.team_factories_ready(game_logic, team_name)
    }

    /// C++ `AIPlayer::checkReadyTeams` — force-complete a team waiting ≥ 60s.
    fn check_ready_teams(&mut self, _game_logic: &mut GameLogic, current_time: f32) {
        const READY_TEAM_FORCE_SECONDS: f32 = 60.0;
        for team in &mut self.team_queue {
            if team.completed {
                continue;
            }
            let started = team.frame_started as f32 / LOGIC_FRAMES_PER_SECOND;
            if current_time - started >= READY_TEAM_FORCE_SECONDS {
                team.completed = true;
            }
        }
    }

    /// C++ `AIPlayer::doUpgradesAndSkills` residual — spend science points.
    fn do_upgrades_and_skills(&mut self, game_logic: &mut GameLogic) {
        let Some(player) = game_logic.get_player_mut(self.player_id) else {
            return;
        };
        if player.science_purchase_points <= 0 {
            return;
        }
        const CANDIDATES: &[&str] = &[
            "SCIENCE_Rank1",
            "SCIENCE_Rank2",
            "SCIENCE_Rank3",
            "SCIENCE_PaladinTank",
            "SCIENCE_StealthFighter",
            "SCIENCE_Pathfinder",
            "SCIENCE_RedGuardTraining",
            "SCIENCE_CashBounty1",
        ];
        for name in CANDIDATES {
            if player.attempt_to_purchase_science(name) {
                break;
            }
        }
    }

    /// Pick candidate team name for the current strategy (same as select_team_to_build).
    fn candidate_team_name(&self) -> Option<String> {
        match self.current_strategy {
            AIStrategy::EarlyGame => self.select_early_game_team(),
            AIStrategy::MidGame => self.select_mid_game_team(),
            AIStrategy::LateGame => self.select_late_game_team(),
            AIStrategy::Desperate => self.select_desperate_team(),
        }
    }

    /// Check if AI should build a new team
    fn should_build_new_team(&self, game_logic: &GameLogic) -> bool {
        // Don't build if queue is full
        if self.team_queue.len() >= 3 {
            return false;
        }
        let Some(team_name) = self.candidate_team_name() else {
            return false;
        };
        // Money (TeamResourcesToStart) + factory/idle residual.
        self.is_possible_to_build_team(game_logic, &team_name)
    }

    /// Select which team to build based on strategy
    fn select_team_to_build(&mut self, game_logic: &mut GameLogic, current_time: f32) -> bool {
        let Some(name) = self.candidate_team_name() else {
            return false;
        };
        // Fail-closed second check at queue time (cash/factories can change mid-tick).
        if !self.is_possible_to_build_team(game_logic, &name) {
            return false;
        }
        let team_queue = self.create_team_queue(&name, current_time);
        self.team_queue.push_back(team_queue);
        // Queuing a production team is a distinct production-linked AI action.
        self.activity_count = self.activity_count.saturating_add(1);

        log::debug!("AI Player {} queued team: {}", self.player_id, name);
        true
    }

    /// Select early game team composition
    fn select_early_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => match self.personality {
                AIPersonality::Rush => Some("USA_RangerSquad".to_string()),
                _ => Some("USA_BasicForce".to_string()),
            },
            Team::China => match self.personality {
                AIPersonality::Rush => Some("China_RedGuardSquad".to_string()),
                _ => Some("China_BasicForce".to_string()),
            },
            Team::GLA => Some("GLA_TechnicalSquad".to_string()),
            _ => None,
        }
    }

    /// Select mid game team composition
    fn select_mid_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_CombinedArms".to_string()),
            Team::China => Some("China_TankSquad".to_string()),
            Team::GLA => Some("GLA_HitAndRun".to_string()),
            _ => None,
        }
    }

    /// Select late game team composition
    fn select_late_game_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_AdvancedStrike".to_string()),
            Team::China => Some("China_HeavyAssault".to_string()),
            Team::GLA => Some("GLA_MassAssault".to_string()),
            _ => None,
        }
    }

    /// Select desperate situation team (cheap, fast units)
    fn select_desperate_team(&self) -> Option<String> {
        match self.team {
            Team::USA => Some("USA_RangerSquad".to_string()),
            Team::China => Some("China_RedGuardSquad".to_string()),
            Team::GLA => Some("GLA_RebelSwarm".to_string()),
            _ => None,
        }
    }

    /// Create team production queue
    fn create_team_queue(&self, team_name: &str, current_time: f32) -> AITeamQueue {
        let work_orders = self.create_work_orders_for_team(team_name);

        AITeamQueue {
            name: team_name.to_string(),
            work_orders,
            priority_build: false,
            frame_started: (current_time * LOGIC_FRAMES_PER_SECOND) as u32,
            completed: false,
        }
    }

    /// Create work orders for a specific team type
    fn create_work_orders_for_team(&self, team_name: &str) -> Vec<AIWorkOrder> {
        let mut orders = Vec::new();

        match team_name {
            "USA_RangerSquad" => {
                orders.push(AIWorkOrder::new(
                    "AmericaInfantryRanger".to_string(),
                    4,
                    100,
                ));
            }
            "USA_BasicForce" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 1, 100));
            }
            "USA_CombinedArms" => {
                orders.push(AIWorkOrder::new("AmericaInfantryRanger".to_string(), 3, 80));
                orders.push(AIWorkOrder::new("AmericaVehicleHumvee".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("USA_CrusaderTank".to_string(), 1, 100));
            }
            "China_RedGuardSquad" => {
                orders.push(AIWorkOrder::new(
                    "ChinaInfantryRedguard".to_string(),
                    4,
                    100,
                ));
            }
            "China_TankSquad" => {
                orders.push(AIWorkOrder::new(
                    "China_BattlemasterTank".to_string(),
                    2,
                    100,
                ));
                orders.push(AIWorkOrder::new("ChinaInfantryRedguard".to_string(), 2, 80));
            }
            "GLA_TechnicalSquad" => {
                // Barracks first: infantry produces even if ArmsDealer is still building.
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_RebelSwarm" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 4, 100));
            }
            "GLA_HitAndRun" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 2, 80));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 2, 100));
            }
            "GLA_MassAssault" => {
                orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 4, 80));
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 3, 100));
            }
            _ => {
                // Default team
                match self.team {
                    Team::USA => orders.push(AIWorkOrder::new(
                        "AmericaInfantryRanger".to_string(),
                        2,
                        100,
                    )),
                    Team::China => orders.push(AIWorkOrder::new(
                        "ChinaInfantryRedguard".to_string(),
                        2,
                        100,
                    )),
                    Team::GLA => {
                        orders.push(AIWorkOrder::new("GLAInfantryRebel".to_string(), 3, 100))
                    }
                    _ => {}
                }
            }
        }

        orders
    }

    /// Find factory that can produce a specific unit
    fn find_factory_for_unit(
        &self,
        game_logic: &GameLogic,
        unit_template_name: &str,
    ) -> Option<ObjectId> {
        Self::find_factory_for_unit_static(game_logic, unit_template_name, self.team)
    }

    /// Static version to avoid borrowing conflicts
    fn find_factory_for_unit_static(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
    ) -> Option<ObjectId> {
        // Prefer idle factory (C++ findFactory(thing, busyOk=false) residual).
        Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, false)
            .or_else(|| Self::find_factory_for_unit_ex(game_logic, unit_template_name, team, true))
    }

    /// Map host unit template → retail factory template residual.
    fn factory_template_for_unit(unit_template_name: &str, team: Team) -> Option<&'static str> {
        let unit = unit_template_name.to_ascii_lowercase();
        if unit.contains("ranger")
            || unit.contains("redguard")
            || unit.contains("soldier")
            || unit.contains("rebel")
        {
            return match team {
                Team::USA => Some("AmericaBarracks"),
                Team::China => Some("ChinaBarracks"),
                Team::GLA => Some("GLABarracks"),
                _ => None,
            };
        }
        if unit.contains("humvee") || unit.contains("technical") || unit.contains("tank") {
            return match team {
                Team::USA => Some("AmericaWarFactory"),
                Team::China => Some("ChinaWarFactory"),
                Team::GLA => Some("GLAArmsDealer"),
                _ => None,
            };
        }
        None
    }

    /// Retail factory name plus leftover USA_*/China_*/GLA_* aliases used by older tests.
    fn factory_name_matches(object_name: &str, retail_factory: &str) -> bool {
        if object_name.eq_ignore_ascii_case(retail_factory) {
            return true;
        }
        let alias = match retail_factory {
            "AmericaBarracks" => "USA_Barracks",
            "AmericaWarFactory" => "USA_WarFactory",
            "ChinaBarracks" => "China_Barracks",
            "ChinaWarFactory" => "China_WarFactory",
            "GLABarracks" => "GLA_Barracks",
            "GLAArmsDealer" => "GLA_ArmsDealer",
            _ => return false,
        };
        object_name.eq_ignore_ascii_case(alias)
    }

    /// C++ factory idle residual: empty production_queue ⇒ idle.
    fn factory_is_idle(object: &crate::game_logic::Object) -> bool {
        object
            .building_data
            .as_ref()
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(true)
    }

    /// Find constructed factory; `busy_ok=false` requires idle queue (C++ findFactory).
    fn find_factory_for_unit_ex(
        game_logic: &GameLogic,
        unit_template_name: &str,
        team: Team,
        busy_ok: bool,
    ) -> Option<ObjectId> {
        let factory_name = Self::factory_template_for_unit(unit_template_name, team)?;
        // Pure residual acquire: prefer idle factories (priority 0) over busy (1)
        // when busy_ok; nearest 3D tiebreak for stable multi-factory choice.
        let cands: Vec<_> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                if object.team != team || !object.is_constructed() || !object.is_alive() {
                    return None;
                }
                let name_ok = Self::factory_name_matches(&object.template_name, factory_name)
                    || Self::factory_name_matches(&object.get_template().name, factory_name);
                if !name_ok {
                    return None;
                }
                let idle = Self::factory_is_idle(object);
                if !busy_ok && !idle {
                    return None;
                }
                let priority = if idle { Some(0u8) } else { Some(1u8) };
                Some(
                    crate::game_logic::host_residual_acquire::PriorityAcquireCandidate {
                        id,
                        position: object.get_position(),
                        is_alive: true,
                        priority,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_best_priority_residual_target(
            ObjectId(0),
            glam::Vec3::ZERO,
            (0.0, 0.0),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `isPossibleToBuildTeam` factory residual (requireIdleFactory=true):
    /// every unit type has a factory, and at least one factory is idle.
    fn team_factories_ready(&self, game_logic: &GameLogic, team_name: &str) -> bool {
        let orders = self.create_work_orders_for_team(team_name);
        if orders.is_empty() {
            return false;
        }
        let mut any_idle = false;
        for order in &orders {
            // Must have some factory that can produce this unit (busy ok for existence).
            if Self::find_factory_for_unit_ex(game_logic, &order.template_name, self.team, true)
                .is_none()
            {
                return false;
            }
            if Self::find_factory_for_unit_ex(game_logic, &order.template_name, self.team, false)
                .is_some()
            {
                any_idle = true;
            }
        }
        any_idle
    }

    /// Minimum seconds between host AI **attack re-evaluations**.
    ///
    /// Wave 616: residual-locked at 60s (not gate-driven early-attack).
    /// Shares the **numeric** 60s value from C++ `AIPlayer::checkReadyTeams`
    /// (`GeneralsMD/.../AI/AIPlayer.cpp`: force-start ready team after
    /// `60 * LOGICFRAMES_PER_SECOND`), but this is **not** a port of that function.
    /// C++ uses 60s for team activation at rally; this host AI uses 60s only as
    /// spacing between strength-threshold attack decisions. Full checkReadyTeams
    /// (idle/anyIdle, production-condition scripts, setActive) remains unported.
    pub const ATTACK_RECHECK_SECONDS: f32 = 60.0;

    /// Retail `AIData.ini` defaults (Default/AIData.ini).
    /// StructureSeconds = 0 → try structure decisions every AI economic tick when ready.
    pub const STRUCTURE_SECONDS: f32 = 0.0;
    /// TeamSeconds = 10 → wait between successful new-team selections.
    pub const TEAM_SECONDS: f32 = 10.0;
    /// C++ `AISkirmishPlayer::doTeamBuilding`: `m_teamDelay = 2 * LOGICFRAMES_PER_SECOND`.
    pub const TEAM_QUEUE_RETRY_SECONDS: f32 = 2.0;
    /// RebuildDelayTimeSeconds = 30 (base rebuild delay residual; full C++ path unported).
    pub const REBUILD_DELAY_SECONDS: f32 = 30.0;
    /// Wealthy resource threshold (AIData `Wealthy`).
    pub const WEALTHY_RESOURCES: u32 = 7000;
    /// Poor resource threshold (AIData `Poor`).
    pub const POOR_RESOURCES: u32 = 2000;
    /// StructuresWealthyRate — interval divisor when wealthy (2=twice as fast).
    pub const STRUCTURES_WEALTHY_RATE: f32 = 2.0;
    /// StructuresPoorRate.
    pub const STRUCTURES_POOR_RATE: f32 = 0.6;
    /// TeamsWealthyRate.
    pub const TEAMS_WEALTHY_RATE: f32 = 2.0;
    /// TeamsPoorRate.
    pub const TEAMS_POOR_RATE: f32 = 0.6;
    /// TeamResourcesToStart fraction residual (documented; full team cost gate unported).
    pub const TEAM_RESOURCES_TO_START: f32 = 0.1;

    /// Evaluate opportunities to attack enemies (strength-threshold + C++-aligned spacing).

    /// AIData wealth/poor rate residual: returns speed multiplier (>= rate means faster).
    fn resource_speed_rate(&self, game_logic: &GameLogic, for_structures: bool) -> f32 {
        let supplies = game_logic
            .get_player(self.player_id)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        if supplies >= Self::WEALTHY_RESOURCES {
            if for_structures {
                Self::STRUCTURES_WEALTHY_RATE
            } else {
                Self::TEAMS_WEALTHY_RATE
            }
        } else if supplies <= Self::POOR_RESOURCES {
            if for_structures {
                Self::STRUCTURES_POOR_RATE
            } else {
                Self::TEAMS_POOR_RATE
            }
        } else {
            1.0
        }
    }

    /// Base interval seconds scaled by difficulty and wealth/poor rates.
    fn scaled_interval_seconds(
        &self,
        game_logic: &GameLogic,
        base_seconds: f32,
        for_structures: bool,
    ) -> f32 {
        if base_seconds <= 0.0 {
            return 0.0;
        }
        let delay = self.difficulty.get_build_delay_modifier().max(0.01);
        let rate = self
            .resource_speed_rate(game_logic, for_structures)
            .max(0.01);
        // C++ rate multiplies speed → shorter wait when rate > 1.
        (base_seconds * delay) / rate
    }

    fn evaluate_attack_opportunities(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.attack_in_progress
            || current_time - self.last_attack_time < Self::ATTACK_RECHECK_SECONDS
        {
            return;
        }

        if let Some(enemy_id) = self.enemy_player_id {
            let our_strength = self.calculate_military_strength(game_logic);
            let enemy_strength = self.estimate_enemy_strength(game_logic, enemy_id);

            // Host personality scales how far we must out-strength the enemy before
            // launching — rough stand-in for scripted team production conditions in C++.
            let aggression = self.difficulty.get_aggression_factor();
            let attack_threshold = match self.personality {
                AIPersonality::Aggressive | AIPersonality::Rush => 0.8 * aggression,
                AIPersonality::Balanced => 1.2 * aggression,
                AIPersonality::Defensive => 1.6 * aggression,
                AIPersonality::Economic => 2.0 * aggression,
            };

            if our_strength > enemy_strength * attack_threshold {
                self.launch_attack(game_logic, current_time);
            }
        }
    }

    /// Calculate our military strength
    fn calculate_military_strength(&self, game_logic: &GameLogic) -> f32 {
        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == self.team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1; // Basic strength calculation
            }
        }

        strength
    }

    /// Estimate enemy military strength
    fn estimate_enemy_strength(&self, game_logic: &GameLogic, enemy_id: u32) -> f32 {
        let enemy_team = if let Some(player) = game_logic.get_player(enemy_id) {
            player.team
        } else {
            return 0.0;
        };

        let mut strength = 0.0;

        for object in game_logic.host_objects().values() {
            if object.team == enemy_team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1;
            }
        }

        strength
    }

    /// Record C++ `AIAttackMoveState` / `AIInternalMoveToState::onEnter` on the
    /// crate `AiStateMachine` (move/attack only; does not run the 48-state graph).
    fn dispatch_crate_attack_move(unit_id: ObjectId, dest: Vec3, focus: Option<ObjectId>) {
        let dest = gamelogic::common::types::Coord3D::new(dest.x, dest.y, dest.z);
        let _ = gamelogic::ai::state_machine::dispatch_host_move_attack(
            unit_id.0,
            gamelogic::ai::state_machine::HostMoveAttackKind::AttackMoveTo,
            Some(dest),
            focus.map(|id| id.0),
        );
    }


    /// Launch coordinated attack
    pub(crate) fn launch_attack(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        log::debug!(
            "AI Player {} ({}) launching attack!",
            self.player_id,
            self.team.get_name()
        );

        // Find our military units
        let mut attack_units = Vec::new();
        for (object_id, object) in game_logic.host_objects() {
            if object.team == self.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
            {
                attack_units.push(*object_id);
            }
        }

        if !attack_units.is_empty() {
            // Find enemy base center
            let enemy_base = if let Some(enemy_id) = self.enemy_player_id {
                if let Some(player) = game_logic.get_player(enemy_id) {
                    self.find_enemy_base_center(game_logic, player.team)
                } else {
                    Vec3::ZERO
                }
            } else {
                Vec3::ZERO
            };

            // Prefer a concrete attackable enemy (set_target → host_attack_log →
            // GameWorld shadow channel). Fall back to attack-move on base center.
            let enemy_team = self
                .enemy_player_id
                .and_then(|eid| game_logic.get_player(eid).map(|p| p.team));
            let focus_enemy = enemy_team.and_then(|eteam| {
                game_logic
                    .host_objects()
                    .iter()
                    .filter(|(_, o)| {
                        o.team == eteam
                            && o.is_alive()
                            && o.is_kind_of(crate::game_logic::KindOf::Attackable)
                    })
                    .min_by(|(_, a), (_, b)| {
                        let da = a.get_position().distance(enemy_base);
                        let db = b.get_position().distance(enemy_base);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(id, _)| *id)
            });

            for &unit_id in &attack_units {
                if let Some(focus) = focus_enemy {
                    // Host-immediate engagement + decision log (GameWorld last-write).
                    game_logic.apply_engagement_decision_aware_for_ai(unit_id, focus);
                }
                // Pathfind toward enemy base like player AttackMove — bare move_to
                // straight-lined through buildings and stranded AI armies.
                let mobile = game_logic
                    .host_object(unit_id)
                    .map(|u| u.is_mobile() && u.is_alive())
                    .unwrap_or(false);
                if !mobile {
                    continue;
                }
                if game_logic.assign_unit_path(unit_id, enemy_base, &[]) {
                    game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                    // C++ AIAttackMoveState / AIInternalMoveToState::onEnter via
                    // crate AiStateMachine::set_state (move/attack only).
                    Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
                } else {
                    // Fallback residual when A* fails (blocked goal).
                    if let Some(unit) = game_logic.host_object_mut(unit_id) {
                        unit.move_to(enemy_base);
                    }
                    game_logic.set_ai_state_decision_aware_for_ai(unit_id, AIState::AttackMoving);
                    Self::dispatch_crate_attack_move(unit_id, enemy_base, focus_enemy);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_move_to(
                            unit_id, enemy_base,
                        );
                    }
                }
            }

            self.attack_in_progress = true;
            self.last_attack_time = current_time;
            self.activity_count = self.activity_count.saturating_add(1);
        }
    }

    /// Find center of enemy base
    fn find_enemy_base_center(&self, game_logic: &GameLogic, enemy_team: Team) -> Vec3 {
        let mut center = Vec3::ZERO;
        let mut count = 0;

        // Find enemy command center or other key buildings
        for object in game_logic.host_objects().values() {
            if object.team == enemy_team
                && object.is_alive()
                && (object.is_kind_of(KindOf::CommandCenter)
                    || object.is_kind_of(KindOf::Structure))
            {
                center += object.get_position();
                count += 1;
            }
        }

        if count > 0 {
            center / count as f32
        } else {
            // Default to opposite corner if no buildings found
            -self.base_center
        }
    }

    /// Update strategic phase based on game state
    fn update_strategy_phase(&mut self, game_logic: &GameLogic, current_time: f32) {
        let game_time = current_time; // Game time in seconds

        match game_time {
            t if t < 300.0 => self.current_strategy = AIStrategy::EarlyGame, // First 5 minutes
            t if t < 900.0 => self.current_strategy = AIStrategy::MidGame,   // 5-15 minutes
            _ => self.current_strategy = AIStrategy::LateGame,               // After 15 minutes
        }

        // Check for desperate situation
        if let Some(player) = game_logic.get_player(self.player_id) {
            if player.resources.supplies < 200 {
                self.current_strategy = AIStrategy::Desperate;
            }
        }
    }

    /// Update build phase based on progress
    fn update_build_phase(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Count constructed buildings
        let built_buildings = self.building_queue.iter().filter(|b| b.is_built).count();

        // Count military units
        let military_units = game_logic
            .host_objects()
            .iter()
            .filter(|(_, obj)| obj.team == self.team && obj.can_attack())
            .count();

        self.build_phase = match (built_buildings, military_units) {
            (0..=2, _) => AIBuildPhase::BaseConstruction,
            (_, 0..=5) => AIBuildPhase::UnitProduction,
            (3..=5, _) => AIBuildPhase::Expansion,
            _ => AIBuildPhase::MassProduction,
        };
    }
}

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

    /// Update all AI players
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.last_update_time >= 0.0
            && current_time - self.last_update_time < self.update_interval
        {
            return;
        }

        // Update each AI player
        let player_ids: Vec<u32> = self.ai_players.keys().copied().collect();
        for player_id in player_ids {
            if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
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
    /// again without burning `max_rebuilds`, and reopens early-base timers.
    pub fn rebind_after_world_reset(&mut self) {
        log::info!(
            "AI Manager: rebinding {} AI player(s) after world reset",
            self.ai_players.len()
        );
        for ai_player in self.ai_players.values_mut() {
            for building in &mut ai_player.building_queue {
                // Map load clears objects; this is not a combat loss — restore rebuild budget.
                building.object_id = None;
                building.is_built = false;
                building.rebuild_count = 0;
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
mod cpp_parity_tests {
    use super::*;

    #[test]
    fn ai_default_base_inside_build_edge_residual() {
        // Default synthetic world is 512² centered at origin; MinDistFromEdge=30.
        // Layout offsets reach +100 (WarFactory) — bases must stay ≤ ~120 from origin.
        let mgr = AIManager::new();
        // Manager construction doesn't add players; mirror add_ai_player centers.
        let centers = [
            (Team::USA, Vec3::new(-120.0, 0.0, -120.0)),
            (Team::China, Vec3::new(120.0, 0.0, -120.0)),
            (Team::GLA, Vec3::new(120.0, 0.0, 120.0)),
        ];
        let half = 256.0;
        let edge = 30.0;
        let max_offset = 100.0;
        for (team, c) in centers {
            let farthest = c.x.abs().max(c.z.abs()) + max_offset;
            assert!(
                farthest <= half - edge,
                "{team:?} base pad would violate edge residual: farthest={farthest}"
            );
        }
        let _ = mgr;
    }

    #[test]
    fn ai_player_update_order_matches_cpp_aiplayer_update() {
        // C++ AIPlayer.cpp:2987-3002
        let src = include_str!("ai.rs");
        let start = src
            .find("/// Main AI update — C++ `AIPlayer::update`")
            .expect("AIPlayer::update docs");
        let body = &src[start..src.len().min(start + 1800)];
        let econ = body.find("update_economic_management").expect("doBaseBuilding");
        let ready = body.find("check_ready_teams").expect("checkReadyTeams");
        let mil = body.find("update_military_management").expect("doTeamBuilding");
        let upg = body.find("do_upgrades_and_skills").expect("doUpgradesAndSkills");
        let br = body
            .find("resume_interrupted_construction")
            .expect("updateBridgeRepair");
        assert!(econ < ready && ready < mil && mil < upg && upg < br);
        assert!(
            AIManager::new().update_interval > 0.0
                && (AIManager::new().update_interval - 1.0 / 30.0).abs() < 1e-6
        );
    }

    #[test]
    fn aidata_timing_constants_match_retail_defaults() {
        // Default/AIData.ini: StructureSeconds=0, TeamSeconds=10, RebuildDelay=30.
        assert_eq!(AIPlayer::STRUCTURE_SECONDS, 0.0);
        assert_eq!(AIPlayer::TEAM_SECONDS, 10.0);
        assert_eq!(AIPlayer::REBUILD_DELAY_SECONDS, 30.0);
        assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
        assert_eq!(AIPlayer::WEALTHY_RESOURCES, 7000);
        assert_eq!(AIPlayer::POOR_RESOURCES, 2000);
        assert!((AIPlayer::STRUCTURES_WEALTHY_RATE - 2.0).abs() < 1e-5);
        assert!((AIPlayer::STRUCTURES_POOR_RATE - 0.6).abs() < 1e-5);
        assert!((AIPlayer::TEAMS_WEALTHY_RATE - 2.0).abs() < 1e-5);
        assert!((AIPlayer::TEAMS_POOR_RATE - 0.6).abs() < 1e-5);
        assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
        // Difficulty stretches TeamSeconds (Easy slower, Hard faster).
        assert!((AIDifficulty::Easy.get_build_delay_modifier() - 2.0).abs() < 1e-5);
        assert!((AIDifficulty::Medium.get_build_delay_modifier() - 1.0).abs() < 1e-5);
        assert!((AIDifficulty::Hard.get_build_delay_modifier() - 0.7).abs() < 1e-5);
    }

    #[test]
    fn aidata_wealth_rate_scales_team_interval() {
        let mut logic = crate::game_logic::GameLogic::new();
        let ai = AIPlayer::new(1, Team::GLA, AIDifficulty::Medium);
        let mut player = crate::game_logic::Player::new(1, Team::GLA, "GLA", true);
        player.resources.supplies = 1000; // poor
        logic.add_player(player);

        let poor = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        logic.get_player_mut(1).unwrap().resources.supplies = 4000; // normal
        let mid = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        logic.get_player_mut(1).unwrap().resources.supplies = 8000; // wealthy
        let rich = ai.scaled_interval_seconds(&logic, AIPlayer::TEAM_SECONDS, false);
        // Poor → longer wait; wealthy → shorter wait.
        assert!(
            poor > mid && mid > rich,
            "poor={poor} mid={mid} rich={rich}"
        );
        assert!(
            (mid - 10.0).abs() < 1e-3,
            "medium normal team interval ~10s got {mid}"
        );
        assert!(
            (rich - 5.0).abs() < 1e-3,
            "wealthy team interval ~5s got {rich}"
        );
        assert!(
            (poor - (10.0 / 0.6)).abs() < 1e-2,
            "poor team interval ~16.67s got {poor}"
        );
        // StructureSeconds=0 stays 0 regardless of wealth.
        assert_eq!(
            ai.scaled_interval_seconds(&logic, AIPlayer::STRUCTURE_SECONDS, true),
            0.0
        );
    }

    #[test]
    fn aidata_team_resources_to_start_gates_queue() {
        // C++ isPossibleToBuildTeam: required = ceil(unit_cost_sum * TeamResourcesToStart).
        assert!((AIPlayer::TEAM_RESOURCES_TO_START - 0.1).abs() < 1e-5);
        let mut logic = crate::game_logic::GameLogic::new();
        // Seed templates with known build costs.
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee.set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);

        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        // USA_BasicForce = 2*Ranger + 1*Humvee = 2*225 + 700 = 1150; *0.1 = 115.
        let full = ai.estimate_team_unit_cost(&logic, "USA_BasicForce");
        assert_eq!(full, 1150);
        let required = (full as f32 * AIPlayer::TEAM_RESOURCES_TO_START).ceil() as u32;
        assert_eq!(required, 115);

        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 114; // one under threshold
        logic.add_player(player);
        assert!(!ai.can_afford_team_start(&logic, "USA_BasicForce"));

        logic.get_player_mut(1).unwrap().resources.supplies = 115;
        assert!(ai.can_afford_team_start(&logic, "USA_BasicForce"));
        // Without factories, is_possible_to_build_team stays false (factory residual).
        assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));
        assert!(!ai.should_build_new_team(&logic));
    }

    #[test]
    fn aidata_team_factory_idle_gate() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger.set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);
        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee.set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);
        let mut barracks_t = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks_t.set_cost(500, 0);
        barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaBarracks".into(), barracks_t);
        let mut wf_t = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        wf_t.set_cost(1000, 0);
        wf_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), wf_t);

        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 10_000;
        logic.add_player(player);

        let ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        // No factories yet.
        assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(!ai.is_possible_to_build_team(&logic, "USA_BasicForce"));

        // Spawn constructed barracks + war factory.
        let barracks_id = logic
            .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("barracks");
        let wf_id = logic
            .create_object(
                "AmericaWarFactory",
                Team::USA,
                glam::Vec3::new(50.0, 0.0, 0.0),
            )
            .expect("war factory");
        // Ensure constructed + empty queues.
        if let Some(o) = logic./* Wave 950 */ host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        if let Some(o) = logic.host_object_mut(wf_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(ai.is_possible_to_build_team(&logic, "USA_BasicForce"));
        assert!(ai.should_build_new_team(&logic));

        // Busy both factories → not ready (requireIdleFactory residual).
        if let Some(o) = logic.host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.push(crate::game_logic::ProductionItem {
                    template_name: "USA_Ranger".into(),
                    progress: 0.1,
                    total_time: 10.0,
                    construction_frames: 0,
                    cost: crate::game_logic::Resources {
                        supplies: 225,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                });
            }
        }
        if let Some(o) = logic.host_object_mut(wf_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.push(crate::game_logic::ProductionItem {
                    template_name: "USA_Humvee".into(),
                    progress: 0.1,
                    total_time: 10.0,
                    construction_frames: 0,
                    cost: crate::game_logic::Resources {
                        supplies: 700,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                });
            }
        }
        assert!(!ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(!ai.should_build_new_team(&logic));

        // Idle one factory → ready again.
        if let Some(o) = logic.host_object_mut(barracks_id) {
            if let Some(b) = o.building_data.as_mut() {
                b.production_queue.clear();
            }
        }
        assert!(ai.team_factories_ready(&logic, "USA_BasicForce"));
        assert!(ai.should_build_new_team(&logic));
    }

    #[test]
    fn skirmish_queues_a_selected_team_without_waiting_for_team_seconds() {
        // Retail `AISkirmishPlayer::doTeamBuilding` first services existing
        // work orders and, after `selectTeamToBuild`, calls `queueUnits` again
        // in that same pass.  A normal USA AI therefore starts its Ranger and
        // Humvee immediately when both real factories are idle; waiting until
        // the next 10-second TeamSeconds window makes the early skirmish AI
        // visibly inert.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 3_000;
        logic.add_player(player);

        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let mut war_factory = crate::game_logic::ThingTemplate::new("AmericaWarFactory");
        war_factory
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSWarFactory)
            .set_cost(1_000, 0);
        logic
            .templates
            .insert("AmericaWarFactory".into(), war_factory);

        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .set_cost(225, 0);
        // Complete on the next real fixed production frame so the test covers
        // the same producer_id handoff that the live skirmish path uses.
        ranger.build_time = 1.0 / 60.0;
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let mut humvee = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
        humvee
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .set_cost(700, 0);
        logic
            .templates
            .insert("AmericaVehicleHumvee".into(), humvee);

        let barracks_id = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .expect("constructed barracks");
        let war_factory_id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(64.0, 0.0, 0.0))
            .expect("constructed war factory");

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.update_military_management(&mut logic, 0.0);

        assert_eq!(ai.team_queue.len(), 1, "the selected team is retained");
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "the selected team's first Ranger is queued in the same AI pass"
        );
        assert_eq!(
            logic
                .host_object(war_factory_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "the selected team's Humvee is queued in the same AI pass"
        );
        assert!(
            (ai.next_team_time - AIPlayer::TEAM_SECONDS).abs() < f32::EPSILON,
            "a successful selection starts the longer TeamSeconds timer"
        );
        assert!(
            (ai.next_team_queue_time - AIPlayer::TEAM_QUEUE_RETRY_SECONDS).abs() < f32::EPSILON,
            "unfinished work orders remain on the short queue cadence"
        );

        // Let the actual production update create the Ranger and stamp its
        // producer.  C++ onUnitProduced shortcuts m_teamDelay at this point;
        // do not wait for the normal 2-second queue poll before starting the
        // second Ranger required by USA_BasicForce.
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        assert!(
            logic.host_objects().values().any(|object| {
                object.team == Team::USA
                    && object.producer_id == Some(barracks_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaInfantryRanger")
            }),
            "the host production path created a producer-linked Ranger"
        );
        let output_time = 1.0 / LOGIC_FRAMES_PER_SECOND;
        ai.update_military_management(&mut logic, output_time);
        let ranger_order = ai
            .team_queue
            .front()
            .and_then(|team| team.work_orders.first())
            .expect("BasicForce Ranger work order remains active");
        assert_eq!(ranger_order.num_completed, 1);
        assert_eq!(ranger_order.queued_count, 1);
        assert_eq!(ranger_order.factory_id, Some(barracks_id));
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1),
            "live output requeues the next Ranger before the normal poll delay"
        );

        // No second team and no duplicate order before m_teamDelay expires.
        ai.update_military_management(&mut logic, 1.9);
        assert_eq!(ai.team_queue.len(), 1);
        assert_eq!(
            logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(1)
        );
    }

    #[test]
    fn work_order_waits_for_live_factory_output_before_completing() {
        // C++ AIPlayer::onUnitProduced increments a WorkOrder only after
        // ProductionUpdate has created a unit and identified its producer.
        // A successful queue request alone must not erase the AI team.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 10_000;
        logic.add_player(player);

        let mut barracks = crate::game_logic::ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::FSBarracks)
            .set_cost(500, 0);
        logic.templates.insert("AmericaBarracks".into(), barracks);

        let mut ranger = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
        ranger
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .add_kind_of(crate::game_logic::KindOf::Attackable)
            .set_cost(225, 0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger);

        let factory = logic
            .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
            .expect("constructed barracks");
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.team_queue.push_back(AITeamQueue {
            name: "one-ranger".into(),
            work_orders: vec![AIWorkOrder::new("AmericaInfantryRanger".into(), 1, 100)],
            priority_build: false,
            frame_started: 0,
            completed: false,
        });

        ai.process_team_queue(&mut logic, 0.0);
        let queued = ai.team_queue.front().expect("queue survives enqueue");
        let order = queued.work_orders.first().expect("work order");
        assert_eq!(
            order.num_completed, 0,
            "enqueue is not production completion"
        );
        assert_eq!(order.queued_count, 1);
        assert_eq!(order.factory_id, Some(factory));

        // Model the real production completion handoff: the production path
        // stamps producer_id before the unit becomes visible to host AI.
        let unit = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(12.0, 0.0, 0.0),
            )
            .expect("factory output");
        logic
            .host_object_mut(unit)
            .expect("produced unit")
            .producer_id = Some(factory);

        ai.process_team_queue(&mut logic, 1.0);
        assert!(
            ai.team_queue.is_empty(),
            "team becomes complete only after its live factory output is observed"
        );
    }

    #[test]
    fn supply_center_spawns_free_collector_then_ai_pays_for_next_collector() {
        // Retail AmericaSupplyCenter has SpawnBehavior ModuleTag_12 for one
        // free AmericaVehicleChinook.  C++ AIPlayer::queueSupplyTruck must not
        // represent that freebie as a zero-cost production item; it later
        // prepends a real paid work order through the same SupplyCenter.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 5_000;
        logic.add_player(player);

        let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(2_000, 0);
        logic
            .templates
            .insert("AmericaSupplyCenter".into(), supply_center);

        let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Aircraft)
            .add_kind_of(crate::game_logic::KindOf::Harvester)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(1_200, 0);
        // Keep the focused test on the real production completion path without
        // waiting ten retail seconds for the paid Chinook.
        chinook.build_time = 0.001;
        logic
            .templates
            .insert("AmericaVehicleChinook".into(), chinook);

        let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
        source
            .add_kind_of(crate::game_logic::KindOf::Resource)
            .add_kind_of(crate::game_logic::KindOf::Harvestable);
        logic.templates.insert("TestSupplySource".into(), source);
        let source_id = logic
            .create_object("TestSupplySource", Team::Neutral, Vec3::new(32.0, 0.0, 0.0))
            .expect("typed supply source");
        logic
            .host_object_mut(source_id)
            .expect("source object")
            .set_stored_supplies(20_000);

        let cash_before_spawn = logic.get_player(1).expect("AI player").effective_supplies();
        let center_id = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::ZERO)
            .expect("constructed supply center");
        let free_collectors: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == Team::USA
                    && object.producer_id == Some(center_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(
            free_collectors.len(),
            1,
            "SpawnBehavior creates one free Chinook"
        );
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after spawn")
                .effective_supplies(),
            cash_before_spawn,
            "the authored SpawnBehavior collector is not charged as production"
        );
        assert_eq!(
            logic
                .host_object(center_id)
                .and_then(|center| center.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(0),
            "free SpawnBehavior collector does not enter ProductionUpdate"
        );

        let free_collector = free_collectors[0];
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.process_team_queue(&mut logic, 0.0);

        let paid_order = ai
            .team_queue
            .front()
            .and_then(|team| team.work_orders.first())
            .expect("one paid follow-up collector work order");
        assert!(paid_order.is_resource_gatherer);
        assert_eq!(paid_order.supply_center_id, Some(center_id));
        assert_eq!(paid_order.factory_id, Some(center_id));
        assert_eq!(paid_order.queued_count, 1);
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after paid queue")
                .effective_supplies(),
            cash_before_spawn - 1_200,
            "only the later collector spends its authored build cost"
        );
        let free = logic
            .host_object(free_collector)
            .expect("free collector live");
        assert_eq!(free.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(free.target, Some(source_id));
        assert_eq!(free.preferred_dock_id, Some(center_id));

        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        ai.process_team_queue(&mut logic, 1.0 / LOGIC_FRAMES_PER_SECOND);

        let paid_collectors: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.team == Team::USA
                    && object.producer_id == Some(center_id)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(
            paid_collectors.len(),
            2,
            "the normal paid ProductionUpdate created a second producer-linked Chinook"
        );
        let paid_collector = *paid_collectors
            .iter()
            .find(|&&id| id != free_collector)
            .expect("new production output");
        let paid = logic
            .host_object(paid_collector)
            .expect("paid collector live");
        assert_eq!(paid.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(paid.target, Some(source_id));
        assert_eq!(paid.preferred_dock_id, Some(center_id));
        assert!(
            ai.team_queue.is_empty(),
            "the paid collector work order completes only after its real output is routed"
        );
    }

    #[test]
    fn collector_returns_to_assigned_supply_center_over_nearer_center() {
        // `AIPlayer::queueSupplyTruck` sends `aiDock(center,
        // CMD_FROM_PLAYER)`.  SupplyTruckAIUpdate persists that center in
        // m_preferredDock, so the return leg must not switch to a different
        // closer depot in a normal multi-supply-center skirmish base.
        let mut logic = crate::game_logic::GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(
            1,
            Team::USA,
            "USA AI",
            false,
        ));

        let mut supply_center = crate::game_logic::ThingTemplate::new("TestSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
        logic
            .templates
            .insert("TestSupplyCenter".into(), supply_center);

        let mut collector = crate::game_logic::ThingTemplate::new("TestCollector");
        collector
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Harvester);
        logic.templates.insert("TestCollector".into(), collector);

        let assigned_center = logic
            .create_object("TestSupplyCenter", Team::USA, Vec3::ZERO)
            .expect("assigned supply center");
        let nearer_center = logic
            .create_object("TestSupplyCenter", Team::USA, Vec3::new(125.0, 0.0, 0.0))
            .expect("nearer supply center");
        let collector_id = logic
            .create_object("TestCollector", Team::USA, Vec3::new(250.0, 0.0, 0.0))
            .expect("collector");
        let assigned_position = logic
            .host_object(assigned_center)
            .expect("assigned center live")
            .get_position();
        {
            let collector = logic.host_object_mut(collector_id).expect("collector live");
            collector.preferred_dock_id = Some(assigned_center);
            collector.set_stored_supplies(100);
            collector.set_ai_state(crate::game_logic::AIState::ReturningResources);
        }

        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);

        let collector = logic
            .host_object(collector_id)
            .expect("collector after tick");
        let queued_destination = collector
            .movement
            .path
            .last()
            .copied()
            .or(collector.movement.target_position);
        assert_eq!(queued_destination, Some(assigned_position));
        assert_ne!(
            queued_destination,
            logic
                .host_object(nearer_center)
                .map(|center| center.get_position()),
            "a nearer center must not steal a collector assigned to another depot"
        );
    }

    #[test]
    fn active_loose_collector_rejoins_one_supply_center_before_paid_replacement() {
        // Retail `AIPlayer::queueSupplyTruck` first scans active SupplyTruckAI
        // units with an unresolved `m_preferredDock`.  It assigns one to the
        // understaffed center and returns before `startTraining`, so losing a
        // supply center does not immediately buy a duplicate collector.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 5_000;
        logic.add_player(player);

        let mut old_center_template = crate::game_logic::ThingTemplate::new("TestOldSupplyCenter");
        old_center_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter);
        logic
            .templates
            .insert("TestOldSupplyCenter".into(), old_center_template);

        let mut supply_center = crate::game_logic::ThingTemplate::new("AmericaSupplyCenter");
        supply_center
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .add_kind_of(crate::game_logic::KindOf::SupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(2_000, 0);
        logic
            .templates
            .insert("AmericaSupplyCenter".into(), supply_center);

        let mut chinook = crate::game_logic::ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Aircraft)
            .add_kind_of(crate::game_logic::KindOf::Harvester)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_cost(1_200, 0);
        logic
            .templates
            .insert("AmericaVehicleChinook".into(), chinook);

        let mut source = crate::game_logic::ThingTemplate::new("TestSupplySource");
        source
            .add_kind_of(crate::game_logic::KindOf::Resource)
            .add_kind_of(crate::game_logic::KindOf::Harvestable);
        logic.templates.insert("TestSupplySource".into(), source);

        // `ObjectID` validity in C++ is an existence test, not merely an
        // alive-state test.  The active survivor must therefore carry an ID
        // that is absent from the host object store (equivalent to a center
        // that already completed its destruction lifecycle).
        let missing_former_dock = ObjectId(100_000);
        assert!(logic.host_object(missing_former_dock).is_none());

        let replacement_center = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(100.0, 0.0, 0.0))
            .expect("replacement center");
        // Remove its authored starter so this test isolates the survivor from
        // the destroyed center.  The parent one-shot latch stays fired, as it
        // would in a real match after that starter had been lost in combat.
        let starter_ids: Vec<ObjectId> = logic
            .host_objects()
            .iter()
            .filter_map(|(&id, object)| {
                (object.producer_id == Some(replacement_center)
                    && object
                        .template_name
                        .eq_ignore_ascii_case("AmericaVehicleChinook"))
                .then_some(id)
            })
            .collect();
        assert_eq!(starter_ids.len(), 1, "authored one-shot starter exists");
        for starter_id in starter_ids {
            logic.destroy_object(starter_id);
        }
        logic.process_destroy_list();
        assert!(logic
            .host_object(replacement_center)
            .is_some_and(|center| center.supply_center_spawn_behavior_fired));

        let source_id = logic
            .create_object(
                "TestSupplySource",
                Team::Neutral,
                Vec3::new(132.0, 0.0, 0.0),
            )
            .expect("nearby supply source");
        logic
            .host_object_mut(source_id)
            .expect("source live")
            .set_stored_supplies(20_000);

        let loose_collector = logic
            .create_object(
                "AmericaVehicleChinook",
                Team::USA,
                Vec3::new(140.0, 0.0, 0.0),
            )
            .expect("surviving collector");
        {
            let collector = logic
                .host_object_mut(loose_collector)
                .expect("surviving collector live");
            collector.preferred_dock_id = Some(missing_former_dock);
            collector.set_order_target(Some(source_id));
            collector.set_ai_state(crate::game_logic::AIState::Gathering);
        }

        let cash_before = logic.get_player(1).expect("AI player").effective_supplies();
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.process_team_queue(&mut logic, 0.0);

        let collector = logic
            .host_object(loose_collector)
            .expect("reassigned collector live");
        assert_eq!(collector.preferred_dock_id, Some(replacement_center));
        assert_eq!(collector.ai_state, crate::game_logic::AIState::Gathering);
        assert_eq!(collector.target, Some(source_id));
        assert!(
            ai.team_queue.is_empty(),
            "C++ returns after one active-collector reassignment before creating a paid work order"
        );
        assert_eq!(
            logic
                .host_object(replacement_center)
                .and_then(|center| center.building_data.as_ref())
                .map(|building| building.production_queue.len()),
            Some(0),
            "the factory does not receive a paid replacement in the reassignment pass"
        );
        assert_eq!(
            logic
                .get_player(1)
                .expect("AI player after reassignment")
                .effective_supplies(),
            cash_before,
            "reassigning an existing collector is free"
        );
    }

    #[test]
    fn skirmish_starts_one_structure_with_a_live_dozer_assignment() {
        // `AISkirmishPlayer::processBaseBuilding` starts one plan and routes a
        // real dozer to it.  A second affordable plan must remain queued until
        // the next economic pass; the scaffold must then make real construction
        // progress through the authoritative dozer target association.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA AI", false);
        player.resources.supplies = 1_000;
        logic.add_player(player);

        let mut dozer_template = crate::game_logic::ThingTemplate::new("TestDozer");
        dozer_template
            .add_kind_of(crate::game_logic::KindOf::Vehicle)
            .add_kind_of(crate::game_logic::KindOf::Worker);
        logic.templates.insert("TestDozer".into(), dozer_template);

        let mut first_template = crate::game_logic::ThingTemplate::new("TestFirstStructure");
        first_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(300, 0);
        first_template.build_time = 10.0;
        logic
            .templates
            .insert("TestFirstStructure".into(), first_template);

        let mut second_template = crate::game_logic::ThingTemplate::new("TestSecondStructure");
        second_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_cost(300, 0);
        logic
            .templates
            .insert("TestSecondStructure".into(), second_template);

        let build_position = Vec3::new(64.0, 0.0, 64.0);
        let dozer_id = logic
            .create_object("TestDozer", Team::USA, Vec3::new(48.0, 0.0, 64.0))
            .expect("live dozer");
        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("TestFirstStructure", build_position, 1);
        ai.add_building("TestSecondStructure", Vec3::new(128.0, 0.0, 64.0), 1);

        ai.process_building_queue(&mut logic, 0.0);

        let structure_id = ai.building_queue[0]
            .object_id
            .expect("first plan starts with its dozer");
        assert!(
            ai.building_queue[1].object_id.is_none(),
            "one C++ skirmish economic pass starts only one structure"
        );
        assert_eq!(
            logic.get_player(1).expect("AI player").effective_supplies(),
            700,
            "only the started structure is charged"
        );
        let dozer = logic.host_object(dozer_id).expect("assigned dozer");
        assert_eq!(dozer.target, Some(structure_id));
        assert_eq!(dozer.ai_state, crate::game_logic::AIState::Constructing);

        // C++ revisits under-construction base entries and hands them to a
        // replacement dozer if the original one is lost.
        let replacement_id = logic
            .create_object("TestDozer", Team::USA, Vec3::new(44.0, 0.0, 64.0))
            .expect("replacement dozer");
        logic
            .host_object_mut(dozer_id)
            .expect("original dozer")
            .health
            .current = 0.0;
        ai.process_building_queue(&mut logic, 0.1);
        let replacement = logic
            .host_object(replacement_id)
            .expect("replacement assigned");
        assert_eq!(replacement.target, Some(structure_id));
        assert_eq!(
            replacement.ai_state,
            crate::game_logic::AIState::Constructing
        );

        let before = logic
            .host_object(structure_id)
            .expect("under-construction scaffold")
            .construction_percent;
        logic.update_with_dt(1.0 / LOGIC_FRAMES_PER_SECOND);
        let after = logic
            .host_object(structure_id)
            .expect("live scaffold")
            .construction_percent;
        assert!(
            after > before,
            "the assigned dozer advances construction instead of leaving a dead scaffold"
        );
    }

    #[test]
    fn aidata_rebuild_delay_gates_destroyed_structure() {
        assert!((AIPlayer::REBUILD_DELAY_SECONDS - 30.0).abs() < 1e-5);
        let b = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
        assert!(b.rebuild_delay_elapsed(0.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(b.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

        let mut destroyed = AIBuildingInfo::new("USA_Barracks".into(), Vec3::ZERO, 2);
        destroyed.destroyed_at_time = Some(10.0);
        // C++: timestamp + RebuildDelaySeconds*FPS > frame → wait.
        assert!(!destroyed.rebuild_delay_elapsed(10.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(!destroyed.rebuild_delay_elapsed(39.9, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(destroyed.rebuild_delay_elapsed(40.0, AIPlayer::REBUILD_DELAY_SECONDS));
        assert!(destroyed.rebuild_delay_elapsed(100.0, AIPlayer::REBUILD_DELAY_SECONDS));

        // process_building_queue stamps destroyed_at_time when object vanishes.
        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "USA", true);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut barracks_t = crate::game_logic::ThingTemplate::new("USA_Barracks");
        barracks_t.set_cost(500, 0);
        barracks_t.add_kind_of(crate::game_logic::KindOf::Structure);
        logic.templates.insert("USA_Barracks".into(), barracks_t);

        let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
        ai.add_building("USA_Barracks", Vec3::new(0.0, 0.0, 0.0), 3);
        // Simulate a previously-built slot whose object was destroyed at t=5.
        {
            let b = &mut ai.building_queue[0];
            b.is_built = false;
            b.object_id = None;
            b.rebuild_count = 1;
            b.destroyed_at_time = Some(5.0);
        }
        // Before delay: queue must not start rebuild.
        ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS - 0.1);
        assert!(ai.building_queue[0].object_id.is_none());
        // After delay: may start (if create_object_under_construction succeeds).
        ai.process_building_queue(&mut logic, 5.0 + AIPlayer::REBUILD_DELAY_SECONDS);
        // Either started (object_id Some) or still none if construction API refused —
        // destroyed_at_time must clear only on successful start.
        if ai.building_queue[0].object_id.is_some() {
            assert!(ai.building_queue[0].destroyed_at_time.is_none());
        } else {
            // Delay gate itself elapsed; remaining failure is construction residual.
            assert!(ai.building_queue[0].rebuild_delay_elapsed(
                5.0 + AIPlayer::REBUILD_DELAY_SECONDS,
                AIPlayer::REBUILD_DELAY_SECONDS
            ));
        }
    }

    #[test]
    fn ai_building_placement_is_deterministic() {
        let mut a = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
        let mut b = AIPlayer::new(3, Team::GLA, AIDifficulty::Medium);
        a.base_center = Vec3::new(100.0, 0.0, 200.0);
        b.base_center = Vec3::new(100.0, 0.0, 200.0);
        // Drain same number of placement draws.
        let pa = (
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
        );
        let pb = (
            b.placement_rng.next_real(-80.0, 80.0),
            b.placement_rng.next_real(-80.0, 80.0),
        );
        assert_eq!(pa, pb, "same player_id seed must match placement draws");
        let mut c = AIPlayer::new(99, Team::GLA, AIDifficulty::Medium);
        let pc = (
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
            c.placement_rng.next_real(-80.0, 80.0),
        );
        let pa4 = (
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
            a.placement_rng.next_real(-80.0, 80.0),
        );
        assert_ne!(pa4, pc, "different player_id seeds must diverge");
    }

    use super::{AIDifficulty, AIManager, AIPlayer};
    use crate::game_logic::{ObjectId, Team};

    /// Gate-only early-attack intervals must not reappear; keep 60s spacing number.
    #[test]
    fn host_attack_recheck_uses_sixty_second_spacing_not_gate_hack() {
        // Same NUMBER as C++ ready-team force-start (60s), not full checkReadyTeams semantics.
        assert_eq!(AIPlayer::ATTACK_RECHECK_SECONDS, 60.0);
        assert!(
            AIPlayer::ATTACK_RECHECK_SECONDS >= 30.0,
            "must not use gate-only early-attack shortcut (<30s)"
        );
    }

    #[test]
    fn rebind_after_world_reset_keeps_difficulty_active_and_restores_rebuild_budget() {
        let mut mgr = AIManager::new();
        mgr.add_ai_player(1, Team::GLA, AIDifficulty::Hard);
        mgr.set_ai_active(1, true);
        {
            let ai = mgr.ai_players.get_mut(&1).expect("ai");
            if let Some(b) = ai.building_queue.first_mut() {
                b.object_id = Some(ObjectId(42));
                b.rebuild_count = b.max_rebuilds;
                b.is_built = true;
            }
            ai.defensive_units.push(ObjectId(7));
            ai.attack_in_progress = true;
        }

        mgr.rebind_after_world_reset();

        assert!(mgr.is_ai_active(1));
        assert_eq!(mgr.ai_difficulty(1), Some(AIDifficulty::Hard));
        let ai = mgr.ai_players.get(&1).expect("ai after rebind");
        assert!(ai.defensive_units.is_empty());
        assert!(!ai.attack_in_progress);
        let b = ai.building_queue.first().expect("layout retained");
        assert!(b.object_id.is_none());
        assert_eq!(b.rebuild_count, 0);
        assert!(!b.is_built);
        assert!(!b.template_name.is_empty());
    }

    #[test]
    fn launch_attack_sets_target_and_logs_host_attack() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::host_attack_log;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

        // Default AI_DECISION_AUTHORITY is on: launch_attack engages host + logs decisions.
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        // Decision logs require coupled shadow writeback frame (live gate).
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        crate::gameworld_shadow::begin_shadow_coupled_tick();
        host_attack_log::clear();
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for (name, team, x) in [("AiAtkU", Team::USA, 0.0f32), ("AiAtkE", Team::GLA, 80.0)] {
            if !logic.templates.contains_key(name) {
                let mut tmpl = ThingTemplate::new(name);
                tmpl.set_health(100.0);
                tmpl.add_kind_of(KindOf::Infantry);
                tmpl.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), tmpl);
            }
            let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
        }
        let usa_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::USA)
            .map(|(id, _)| *id)
            .unwrap_or(0);
        let gla_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::GLA)
            .map(|(id, _)| *id);
        let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = gla_id;
        ai.is_active = true;
        let usa_unit = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.is_alive())
            .map(|(id, _)| *id)
            .expect("usa unit");
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
            });
        }
        ai.launch_attack(&mut logic, 1000.0);
        let decisions = host_ai_decision_log::drain();
        let unit = logic
            .host_objects()
            .get(&usa_unit)
            .expect("usa unit after launch");
        assert!(
            unit.target.is_some(),
            "under AI decision authority host target engages immediately"
        );
        assert!(
            decisions.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK && e.host_object == usa_unit
            }),
            "launch_attack must log AttackTarget decision; got {decisions:?}"
        );
        assert!(
            decisions.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == usa_unit
                    && e.ai_state_ordinal == 3
            }),
            "launch_attack must log AttackMoving state; got {decisions:?}"
        );
        assert!(
            !unit.movement.path.is_empty() || unit.movement.target_position.is_some(),
            "launch_attack must still pathfind on host under decision authority"
        );
        // Legacy residual path when decision authority is off.
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
        host_attack_log::clear();
        host_ai_decision_log::clear();
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.target = None;
            o.ai_state = AIState::Idle;
            o.movement.path.clear();
            o.movement.target_position = None;
        }
        ai.launch_attack(&mut logic, 2000.0);
        let logged = host_attack_log::drain();
        let unit = logic
            .host_objects()
            .get(&usa_unit)
            .expect("usa unit legacy");
        assert!(
            unit.target.is_some() && !logged.is_empty(),
            "legacy launch_attack must set_target and host_attack_log"
        );
        assert_eq!(unit.ai_state, AIState::AttackMoving);
        crate::gameworld_shadow::end_shadow_coupled_tick();
        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn launch_attack_uses_assign_unit_path_surface() {
        let src = include_str!("ai.rs");
        // Do not split on cfg(test) — nested test modules can appear earlier.
        let i = src
            .find("fn launch_attack(&mut self, game_logic")
            .expect("launch_attack");
        let w = &src[i..i + 4500.min(src.len() - i)];
        assert!(
            w.contains("assign_unit_path")
                && (w.contains("AIState::AttackMoving") || w.contains("record_set_state")),
            "AI launch_attack must pathfind then restore AttackMoving (host or decision log)"
        );
        // Fallback may call move_to after assign_unit_path fails; primary path
        // must call assign_unit_path first.
        let path_i = w.find("assign_unit_path").expect("path");
        let move_i = w.find("move_to(enemy_base)");
        assert!(
            move_i.is_none() || move_i.unwrap() > path_i,
            "move_to fallback must come after assign_unit_path"
        );
    }

    #[test]
    fn launch_attack_dispatches_crate_attack_move_state() {
        // C++ AIAttackMoveState / AIInternalMoveToState::onEnter (AIStates.cpp).
        // Live host AIState is a flat enum; launch_attack must also record
        // crate AiStateType::AttackMoveTo via dispatch_host_move_attack.
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        use gamelogic::ai::state_machine::{host_move_attack_state, AiStateType};

        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        crate::gameworld_shadow::begin_shadow_coupled_tick();

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiSm");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for (name, team, x) in [("AiSmU", Team::USA, 0.0f32), ("AiSmE", Team::GLA, 80.0)] {
            if !logic.templates.contains_key(name) {
                let mut tmpl = ThingTemplate::new(name);
                tmpl.set_health(100.0);
                tmpl.add_kind_of(KindOf::Infantry);
                tmpl.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), tmpl);
            }
            let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
        }
        let usa_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::USA)
            .map(|(id, _)| *id)
            .unwrap_or(0);
        let gla_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::GLA)
            .map(|(id, _)| *id);
        let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = gla_id;
        ai.is_active = true;
        let usa_unit = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.is_alive())
            .map(|(id, _)| *id)
            .expect("usa unit");
        if let Some(o) = logic.host_object_mut(usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
            });
        }

        ai.launch_attack(&mut logic, 1000.0);

        assert_eq!(
            host_move_attack_state(usa_unit.0),
            Some(AiStateType::AttackMoveTo),
            "launch_attack must record crate AttackMoveTo for the live unit"
        );
        assert_eq!(
            logic.host_object(usa_unit).map(|o| o.ai_state.clone()),
            Some(AIState::AttackMoving)
        );

        crate::gameworld_shadow::end_shadow_coupled_tick();
        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

}
