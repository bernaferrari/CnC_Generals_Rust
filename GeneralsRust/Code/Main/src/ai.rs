use crate::game_logic::*;
use glam::Vec3;
use rand::Rng;
use std::collections::{HashMap, VecDeque};

/// AI difficulty levels affecting decision making and timing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub num_completed: u32,
    pub num_required: u32,
    pub is_required: bool,
    pub priority: u32,
}

impl AIWorkOrder {
    pub fn new(template_name: String, count: u32, priority: u32) -> Self {
        Self {
            template_name,
            factory_id: None,
            num_completed: 0,
            num_required: count,
            is_required: true,
            priority,
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
    pub building_queue: Vec<AIBuildingInfo>,
    pub next_building_time: f32,
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
            building_queue: Vec::new(),
            next_building_time: 0.0,
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
        }
    }

    /// Initialize AI with starting base layout
    pub fn initialize(&mut self, base_position: Vec3) {
        self.base_center = base_position;
        self.setup_base_layout();
        self.setup_initial_strategy();
    }

    /// Main AI update method - called every frame
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if !self.is_active {
            return;
        }

        self.last_update_time = current_time;

        // Update AI systems in order
        self.update_enemy_assessment(game_logic, current_time);
        self.update_economic_management(game_logic, current_time);
        self.update_military_management(game_logic, current_time);
        self.update_strategic_decisions(game_logic, current_time);
    }

    /// Set up initial base building layout
    fn setup_base_layout(&mut self) {
        let center = self.base_center;

        // Core base buildings based on team
        match self.team {
            Team::USA => {
                self.add_building("USA_CommandCenter", center, 1);
                self.add_building("USA_SupplyCenter", center + Vec3::new(50.0, 0.0, 0.0), 2);
                self.add_building("USA_PowerPlant", center + Vec3::new(-50.0, 0.0, 0.0), 2);
                self.add_building("USA_Barracks", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("USA_WarFactory", center + Vec3::new(100.0, 0.0, 50.0), 1);
            }
            Team::China => {
                self.add_building("China_CommandCenter", center, 1);
                self.add_building("China_SupplyCenter", center + Vec3::new(50.0, 0.0, 0.0), 2);
                self.add_building("China_PowerPlant", center + Vec3::new(-50.0, 0.0, 0.0), 2);
                self.add_building("China_Barracks", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("China_WarFactory", center + Vec3::new(100.0, 0.0, 50.0), 1);
            }
            Team::GLA => {
                self.add_building("GLA_CommandCenter", center, 1);
                self.add_building("GLA_SupplyStash", center + Vec3::new(50.0, 0.0, 0.0), 3);
                self.add_building("GLA_ArmsDealer", center + Vec3::new(0.0, 0.0, 50.0), 2);
                self.add_building("GLA_Barracks", center + Vec3::new(-50.0, 0.0, 50.0), 2);
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

        // Set initial timing based on difficulty
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_building_time = self.last_update_time + (10.0 * delay_modifier);
        self.next_team_time = self.last_update_time + (20.0 * delay_modifier);
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
                println!(
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

        // Process building queue
        self.process_building_queue(game_logic, current_time);

        // Set next building check time
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_building_time = current_time + (8.0 * delay_modifier);
    }

    /// Update military management (unit production, attack coordination)
    fn update_military_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if current_time < self.next_team_time {
            return;
        }

        // Process team production queue
        self.process_team_queue(game_logic, current_time);

        // Decide if we should build new teams
        if self.should_build_new_team(game_logic) {
            self.select_team_to_build(game_logic, current_time);
        }

        // Check for attack opportunities
        self.evaluate_attack_opportunities(game_logic, current_time);

        // Set next team check time
        let delay_modifier = self.difficulty.get_build_delay_modifier();
        self.next_team_time = current_time + (15.0 * delay_modifier);
    }

    /// Update strategic decisions and long-term planning
    fn update_strategic_decisions(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update strategy based on game state
        self.update_strategy_phase(game_logic, current_time);

        // Update build phase
        self.update_build_phase(game_logic, current_time);
    }

    /// Process building construction queue
    fn process_building_queue(&mut self, game_logic: &mut GameLogic, _current_time: f32) {
        if let Some(player) = game_logic.get_player(self.player_id) {
            // Find next building to construct
            let mut build_index: Option<usize> = None;

            for (i, building) in self.building_queue.iter().enumerate() {
                if !building.is_built && building.rebuild_count < building.max_rebuilds {
                    // Check if we can afford it
                    if let Some(template) = game_logic.templates.get(&building.template_name) {
                        if player.can_afford(&Resources {
                            supplies: template.build_cost.supplies,
                            power: template.build_cost.power,
                        }) {
                            build_index = Some(i);
                            break;
                        }
                    }
                }
            }

            if let Some(index) = build_index {
                let building = &mut self.building_queue[index];

                // Try to build the structure
                if let Some(object_id) = game_logic.create_object_under_construction(
                    &building.template_name,
                    self.team,
                    building.position,
                ) {
                    building.object_id = Some(object_id);
                    building.rebuild_count += 1;

                    // Spend the resources
                    let cost = game_logic
                        .templates
                        .get(&building.template_name)
                        .map(|template| template.build_cost);
                    if let Some(build_cost) = cost {
                        if let Some(player) = game_logic.get_player_mut(self.player_id) {
                            player.spend_resources(&build_cost);
                        }
                    }

                    println!(
                        "AI Player {} building {} at {:?}",
                        self.player_id, building.template_name, building.position
                    );
                }
            }
        }

        // Update building status
        for building in &mut self.building_queue {
            if let Some(object_id) = building.object_id {
                if let Some(object) = game_logic.find_object(object_id) {
                    building.is_built = object.is_constructed();
                } else {
                    // Building was destroyed
                    building.object_id = None;
                    building.is_built = false;
                }
            }
        }
    }

    /// Try to build a supply center for resource generation
    fn try_build_supply_center(&mut self, _game_logic: &mut GameLogic) {
        let supply_center_name = match self.team {
            Team::USA => "USA_SupplyCenter",
            Team::China => "China_SupplyCenter",
            Team::GLA => "GLA_SupplyStash",
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
                    rand::thread_rng().gen_range(-80.0..80.0),
                    0.0,
                    rand::thread_rng().gen_range(-80.0..80.0),
                );

            self.add_building(supply_center_name, position, 2);
        }
    }

    /// Try to build a power plant for energy
    fn try_build_power_plant(&mut self, _game_logic: &mut GameLogic) {
        let power_plant_name = match self.team {
            Team::USA => "USA_PowerPlant",
            Team::China => "China_PowerPlant",
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
                    rand::thread_rng().gen_range(-60.0..60.0),
                    0.0,
                    rand::thread_rng().gen_range(-60.0..60.0),
                );

            self.add_building(power_plant_name, position, 1);
        }
    }

    /// Process team production queue
    fn process_team_queue(&mut self, game_logic: &mut GameLogic, _current_time: f32) {
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

        // Process factory assignments
        for (team_index, order_index, template_name) in factory_assignments {
            if let Some(factory_id) =
                Self::find_factory_for_unit_static(game_logic, &template_name, self.team)
            {
                if let Some(team) = self.team_queue.get_mut(team_index) {
                    if let Some(work_order) = team.work_orders.get_mut(order_index) {
                        work_order.factory_id = Some(factory_id);
                    }
                }
            }
        }

        // Remove completed teams
        for &index in completed_teams.iter().rev() {
            if let Some(team) = self.team_queue.remove(index) {
                println!("AI Player {} completed team: {}", self.player_id, team.name);
            }
        }
    }

    /// Check if AI should build a new team
    fn should_build_new_team(&self, game_logic: &GameLogic) -> bool {
        // Don't build if queue is full
        if self.team_queue.len() >= 3 {
            return false;
        }

        // Check if we have resources for a basic team
        if let Some(player) = game_logic.get_player(self.player_id) {
            let min_resources = match self.difficulty {
                AIDifficulty::Easy => 300,
                AIDifficulty::Medium => 500,
                AIDifficulty::Hard => 800,
                AIDifficulty::Brutal => 1000,
            };

            player.resources.supplies >= min_resources
        } else {
            false
        }
    }

    /// Select which team to build based on strategy
    fn select_team_to_build(&mut self, _game_logic: &mut GameLogic, current_time: f32) {
        let team_name = match self.current_strategy {
            AIStrategy::EarlyGame => self.select_early_game_team(),
            AIStrategy::MidGame => self.select_mid_game_team(),
            AIStrategy::LateGame => self.select_late_game_team(),
            AIStrategy::Desperate => self.select_desperate_team(),
        };

        if let Some(name) = team_name {
            let team_queue = self.create_team_queue(&name, current_time);
            self.team_queue.push_back(team_queue);

            println!("AI Player {} queued team: {}", self.player_id, name);
        }
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
            frame_started: (current_time * 60.0) as u32, // Convert to frame
            completed: false,
        }
    }

    /// Create work orders for a specific team type
    fn create_work_orders_for_team(&self, team_name: &str) -> Vec<AIWorkOrder> {
        let mut orders = Vec::new();

        match team_name {
            "USA_RangerSquad" => {
                orders.push(AIWorkOrder::new("USA_Ranger".to_string(), 4, 100));
            }
            "USA_BasicForce" => {
                orders.push(AIWorkOrder::new("USA_Ranger".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("USA_Humvee".to_string(), 1, 100));
            }
            "USA_CombinedArms" => {
                orders.push(AIWorkOrder::new("USA_Ranger".to_string(), 3, 80));
                orders.push(AIWorkOrder::new("USA_Humvee".to_string(), 2, 90));
                orders.push(AIWorkOrder::new("USA_CrusaderTank".to_string(), 1, 100));
            }
            "China_RedGuardSquad" => {
                orders.push(AIWorkOrder::new("China_RedGuard".to_string(), 4, 100));
            }
            "China_TankSquad" => {
                orders.push(AIWorkOrder::new(
                    "China_BattlemasterTank".to_string(),
                    2,
                    100,
                ));
                orders.push(AIWorkOrder::new("China_RedGuard".to_string(), 2, 80));
            }
            "GLA_TechnicalSquad" => {
                orders.push(AIWorkOrder::new("GLA_Technical".to_string(), 3, 100));
            }
            _ => {
                // Default team
                match self.team {
                    Team::USA => orders.push(AIWorkOrder::new("USA_Ranger".to_string(), 2, 100)),
                    Team::China => {
                        orders.push(AIWorkOrder::new("China_RedGuard".to_string(), 2, 100))
                    }
                    Team::GLA => orders.push(AIWorkOrder::new("GLA_Soldier".to_string(), 3, 100)),
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
        // Map units to their production buildings
        let factory_name = match unit_template_name {
            s if s.contains("Ranger") || s.contains("RedGuard") || s.contains("Soldier") => {
                match team {
                    Team::USA => "USA_Barracks",
                    Team::China => "China_Barracks",
                    Team::GLA => "GLA_Barracks",
                    _ => return None,
                }
            }
            s if s.contains("Humvee") || s.contains("Technical") || s.contains("Tank") => {
                match team {
                    Team::USA => "USA_WarFactory",
                    Team::China => "China_WarFactory",
                    Team::GLA => "GLA_ArmsDealer",
                    _ => return None,
                }
            }
            _ => return None,
        };

        // Find a constructed factory
        for (object_id, object) in game_logic.get_objects() {
            if object.team == team
                && object.get_template().name == factory_name
                && object.is_constructed()
                && object.is_alive()
            {
                return Some(*object_id);
            }
        }

        None
    }

    /// Evaluate opportunities to attack enemies
    fn evaluate_attack_opportunities(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if self.attack_in_progress || current_time - self.last_attack_time < 60.0 {
            return; // Don't attack too frequently
        }

        if let Some(enemy_id) = self.enemy_player_id {
            let our_strength = self.calculate_military_strength(game_logic);
            let enemy_strength = self.estimate_enemy_strength(game_logic, enemy_id);

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

        for object in game_logic.get_objects().values() {
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

        for object in game_logic.get_objects().values() {
            if object.team == enemy_team && object.is_alive() && object.can_attack() {
                strength += object.health.current * 0.1;
            }
        }

        strength
    }

    /// Launch coordinated attack
    fn launch_attack(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        println!(
            "AI Player {} ({}) launching attack!",
            self.player_id,
            self.team.get_name()
        );

        // Find our military units
        let mut attack_units = Vec::new();
        for (object_id, object) in game_logic.get_objects() {
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

            // Command all units to attack-move to enemy base
            for &unit_id in &attack_units {
                if let Some(unit) = game_logic.find_object_mut(unit_id) {
                    unit.move_to(enemy_base);
                    unit.ai_state = AIState::AttackMoving;
                }
            }

            self.attack_in_progress = true;
            self.last_attack_time = current_time;
        }
    }

    /// Find center of enemy base
    fn find_enemy_base_center(&self, game_logic: &GameLogic, enemy_team: Team) -> Vec3 {
        let mut center = Vec3::ZERO;
        let mut count = 0;

        // Find enemy command center or other key buildings
        for object in game_logic.get_objects().values() {
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
            .get_objects()
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
            update_interval: 1.0 / 10.0, // Update AI at 10 FPS
            last_update_time: 0.0,
        }
    }

    /// Add AI player
    pub fn add_ai_player(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        let mut ai_player = AIPlayer::new(player_id, team, difficulty);

        // Initialize with team-appropriate base position
        let base_position = match team {
            Team::USA => Vec3::new(-200.0, 0.0, -200.0),
            Team::China => Vec3::new(200.0, 0.0, -200.0),
            Team::GLA => Vec3::new(200.0, 0.0, 200.0),
            _ => Vec3::ZERO,
        };

        ai_player.initialize(base_position);
        self.ai_players.insert(player_id, ai_player);

        println!(
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
        if current_time - self.last_update_time < self.update_interval {
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

    /// Enable/disable AI for a player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        if let Some(ai_player) = self.ai_players.get_mut(&player_id) {
            ai_player.is_active = active;
        }
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

    /// Called when a game is loaded from save
    pub fn on_game_loaded(&mut self) {
        println!("AI Manager: Game loaded, reinitializing AI state...");

        // Clear any transient state and reinitialize
        for ai_player in self.ai_players.values_mut() {
            // Reset timing-based state
            ai_player.last_update_time = 0.0;
            ai_player.resource_check_time = 0.0;
            ai_player.enemy_check_time = 0.0;
            ai_player.next_building_time = 0.0;
            ai_player.next_team_time = 0.0;
            ai_player.last_attack_time = 0.0;

            // Clear temporary lists that shouldn't persist
            ai_player.defensive_units.clear();

            println!(
                "  Reinitialized AI player {} ({})",
                ai_player.player_id,
                ai_player.team.get_name()
            );
        }

        self.last_update_time = 0.0;
        println!("AI Manager: Game load initialization complete");
    }

    /// Clear all pending AI commands
    pub fn clear_pending_commands(&mut self) {
        println!("AI Manager: Clearing all pending commands...");

        for ai_player in self.ai_players.values_mut() {
            // Clear building queues
            ai_player.building_queue.clear();

            // Clear team queues
            ai_player.team_queue.clear();

            // Reset attack state
            ai_player.attack_in_progress = false;

            println!(
                "  Cleared commands for AI player {} ({})",
                ai_player.player_id,
                ai_player.team.get_name()
            );
        }

        println!("AI Manager: All pending commands cleared");
    }
}
