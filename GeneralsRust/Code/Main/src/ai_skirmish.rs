use crate::ai::*;
use crate::game_logic::*;
use glam::Vec3;
use std::collections::{HashMap, VecDeque};

/// Advanced AI Skirmish Player with sophisticated strategic behavior
#[derive(Debug)]
pub struct AISkirmishPlayer {
    /// Base AI Player functionality
    pub base: AIPlayer,

    /// Advanced base defense management
    pub front_base_defense_count: u32,
    pub flank_base_defense_count: u32,
    pub defense_angles: BaseDefenseAngles,

    /// Superweapon and special power management
    pub available_superweapons: Vec<String>,
    pub last_superweapon_use: f32,
    pub superweapon_targets: Vec<Vec3>,

    /// Advanced pathfinding and movement
    pub bridge_repair_queue: VecDeque<ObjectId>,
    pub dozer_assignments: HashMap<ObjectId, ObjectId>, // Dozer ID -> Building ID

    /// Strategic command and control
    pub rally_points: HashMap<String, Vec3>,
    pub attack_groups: Vec<AttackGroup>,
    pub scout_units: Vec<ObjectId>,

    /// Advanced economic management
    pub supply_route_security: Vec<ObjectId>,
    pub expansion_sites: Vec<ExpansionSite>,
    pub economic_focus: EconomicFocus,

    /// Tactical combat management
    pub combat_groups: HashMap<String, CombatGroup>,
    pub defensive_positions: Vec<DefensivePosition>,
    pub retreat_positions: Vec<Vec3>,

    /// Strategic intelligence
    pub enemy_intel: HashMap<u32, EnemyIntelligence>,
    pub scouted_areas: Vec<ScoutedArea>,
    pub threat_assessment: ThreatLevel,
}

/// Base defense angle management for positioning defensive structures
#[derive(Debug, Default)]
pub struct BaseDefenseAngles {
    pub front_left: f32,
    pub front_right: f32,
    pub left_flank_left: f32,
    pub left_flank_right: f32,
    pub right_flank_left: f32,
    pub right_flank_right: f32,
}

/// Attack group coordination
#[derive(Debug, Clone)]
pub struct AttackGroup {
    pub name: String,
    pub units: Vec<ObjectId>,
    pub objective: AttackObjective,
    pub formation: Formation,
    pub status: GroupStatus,
    pub rally_point: Vec3,
    pub target_position: Vec3,
}

/// Attack objectives
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackObjective {
    DestroyBase,
    HarassEconomy,
    SecureArea,
    DefendBase,
    Reconnaissance,
}

/// Military formations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formation {
    Line,
    Column,
    Wedge,
    Box,
    Scatter,
}

/// Group status tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStatus {
    Forming,
    Moving,
    Engaging,
    Regrouping,
    Retreating,
    Disbanded,
}

/// Expansion site management
#[derive(Debug, Clone)]
pub struct ExpansionSite {
    pub position: Vec3,
    pub resource_value: f32,
    pub security_level: f32,
    pub is_occupied: bool,
    pub assigned_builders: Vec<ObjectId>,
}

/// Economic focus strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EconomicFocus {
    Balanced,
    SupplyRush,
    TechRush,
    MassProduction,
    Defensive,
}

/// Combat group management
#[derive(Debug, Clone)]
pub struct CombatGroup {
    pub units: Vec<ObjectId>,
    pub role: CombatRole,
    pub experience_level: f32,
    pub morale: f32,
    pub effectiveness: f32,
}

/// Combat roles for specialized groups
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatRole {
    Infantry,
    Armor,
    Artillery,
    AntiAir,
    Support,
    Elite,
}

/// Defensive positions and strongpoints
#[derive(Debug, Clone)]
pub struct DefensivePosition {
    pub position: Vec3,
    pub importance: f32,
    pub defending_units: Vec<ObjectId>,
    pub defensive_structures: Vec<ObjectId>,
    pub threat_level: f32,
}

/// Enemy intelligence gathering and analysis
#[derive(Debug, Clone)]
pub struct EnemyIntelligence {
    pub player_id: u32,
    pub team: Team,
    pub base_locations: Vec<Vec3>,
    pub military_strength: f32,
    pub economic_strength: f32,
    pub tech_level: u32,
    pub recent_activities: Vec<EnemyActivity>,
    pub threat_assessment: f32,
}

/// Enemy activity tracking
#[derive(Debug, Clone)]
pub struct EnemyActivity {
    pub activity_type: ActivityType,
    pub location: Vec3,
    pub timestamp: f32,
    pub units_involved: u32,
}

/// Types of enemy activities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityType {
    BaseExpansion,
    MilitaryBuildup,
    Attack,
    Patrol,
    Retreat,
    Construction,
}

/// Scouted area information
#[derive(Debug, Clone)]
pub struct ScoutedArea {
    pub center: Vec3,
    pub radius: f32,
    pub last_scouted: f32,
    pub enemy_presence: bool,
    pub strategic_value: f32,
}

/// Overall threat level assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatLevel {
    Minimal,
    Low,
    Moderate,
    High,
    Critical,
}

impl AISkirmishPlayer {
    /// Create new AI Skirmish Player with advanced capabilities
    pub fn new(player_id: u32, team: Team, difficulty: AIDifficulty) -> Self {
        Self {
            base: AIPlayer::new(player_id, team, difficulty),
            front_base_defense_count: 0,
            flank_base_defense_count: 0,
            defense_angles: BaseDefenseAngles::default(),
            available_superweapons: Vec::new(),
            last_superweapon_use: 0.0,
            superweapon_targets: Vec::new(),
            bridge_repair_queue: VecDeque::new(),
            dozer_assignments: HashMap::new(),
            rally_points: HashMap::new(),
            attack_groups: Vec::new(),
            scout_units: Vec::new(),
            supply_route_security: Vec::new(),
            expansion_sites: Vec::new(),
            economic_focus: EconomicFocus::Balanced,
            combat_groups: HashMap::new(),
            defensive_positions: Vec::new(),
            retreat_positions: Vec::new(),
            enemy_intel: HashMap::new(),
            scouted_areas: Vec::new(),
            threat_assessment: ThreatLevel::Low,
        }
    }

    /// Initialize advanced AI systems
    pub fn initialize(&mut self, base_position: Vec3) {
        self.base.initialize(base_position);
        self.setup_advanced_systems(base_position);
    }

    /// Set up advanced AI systems
    fn setup_advanced_systems(&mut self, base_position: Vec3) {
        // Set up rally points
        self.rally_points.insert(
            "main".to_string(),
            base_position + Vec3::new(0.0, 0.0, -50.0),
        );
        self.rally_points.insert(
            "secondary".to_string(),
            base_position + Vec3::new(50.0, 0.0, -50.0),
        );
        self.rally_points.insert(
            "fallback".to_string(),
            base_position + Vec3::new(-50.0, 0.0, -50.0),
        );

        // Set up retreat positions
        self.retreat_positions
            .push(base_position + Vec3::new(-100.0, 0.0, -100.0));
        self.retreat_positions
            .push(base_position + Vec3::new(100.0, 0.0, -100.0));

        // Set up initial expansion sites
        let expansion_radius = 200.0;
        for i in 0..4 {
            let angle = (i as f32) * std::f32::consts::PI / 2.0;
            let pos = base_position
                + Vec3::new(
                    expansion_radius * angle.cos(),
                    0.0,
                    expansion_radius * angle.sin(),
                );

            self.expansion_sites.push(ExpansionSite {
                position: pos,
                resource_value: 100.0,
                security_level: 50.0,
                is_occupied: false,
                assigned_builders: Vec::new(),
            });
        }

        // Initialize defensive positions around base
        self.setup_defensive_positions(base_position);

        // Set up team-specific superweapons
        self.setup_superweapons();
    }

    /// Set up defensive positions around the base
    fn setup_defensive_positions(&mut self, base_position: Vec3) {
        let defense_radius = 120.0;
        let positions = 8;

        for i in 0..positions {
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / (positions as f32);
            let pos = base_position
                + Vec3::new(
                    defense_radius * angle.cos(),
                    0.0,
                    defense_radius * angle.sin(),
                );

            let importance = match i {
                0 | 1 => 1.0, // Front defenses (most important)
                2 | 6 => 0.8, // Flank defenses
                3 | 5 => 0.6, // Side defenses
                4 | 7 => 0.4, // Rear defenses (least important)
                _ => 0.5,
            };

            self.defensive_positions.push(DefensivePosition {
                position: pos,
                importance,
                defending_units: Vec::new(),
                defensive_structures: Vec::new(),
                threat_level: 0.0,
            });
        }
    }

    /// Set up available superweapons based on team
    fn setup_superweapons(&mut self) {
        match self.base.team {
            Team::USA => {
                self.available_superweapons.push("ParticleBeam".to_string());
                self.available_superweapons.push("A10Strike".to_string());
                self.available_superweapons.push("FuelAirBomb".to_string());
            }
            Team::China => {
                self.available_superweapons
                    .push("NuclearMissile".to_string());
                self.available_superweapons
                    .push("ArtilleryBarrage".to_string());
                self.available_superweapons
                    .push("EmergencyRepair".to_string());
            }
            Team::GLA => {
                self.available_superweapons.push("ScudStorm".to_string());
                self.available_superweapons.push("AnthraxBomb".to_string());
                self.available_superweapons.push("SneakAttack".to_string());
            }
            _ => {}
        }
    }

    /// Main update method with advanced AI systems
    pub fn update(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        if !self.base.is_active {
            return;
        }

        // Update base AI functionality
        self.base.update(game_logic, current_time);

        // Update advanced systems
        self.update_intelligence_gathering(game_logic, current_time);
        self.update_base_defense_management(game_logic, current_time);
        self.update_attack_coordination(game_logic, current_time);
        self.update_economic_optimization(game_logic, current_time);
        self.update_superweapon_management(game_logic, current_time);
        self.update_tactical_combat(game_logic, current_time);
    }

    /// Update intelligence gathering and enemy assessment
    fn update_intelligence_gathering(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update enemy intelligence every 10 seconds
        if (current_time * 10.0) as u32 % 100 != 0 {
            return;
        }

        // Gather intelligence on all enemy players
        for player_id in 0..4 {
            if player_id == self.base.player_id {
                continue;
            }

            if let Some(player) = game_logic.get_player(player_id) {
                if player.team != self.base.team && player.is_alive {
                    self.analyze_enemy_player(game_logic, player_id, current_time);
                }
            }
        }

        // Update overall threat assessment
        self.update_threat_assessment();

        // Deploy scouts if needed
        self.manage_reconnaissance(game_logic, current_time);
    }

    /// Analyze specific enemy player
    fn analyze_enemy_player(&mut self, game_logic: &GameLogic, enemy_id: u32, _current_time: f32) {
        if let Some(player) = game_logic.get_player(enemy_id) {
            let intel = self
                .enemy_intel
                .entry(enemy_id)
                .or_insert(EnemyIntelligence {
                    player_id: enemy_id,
                    team: player.team,
                    base_locations: Vec::new(),
                    military_strength: 0.0,
                    economic_strength: 0.0,
                    tech_level: 1,
                    recent_activities: Vec::new(),
                    threat_assessment: 0.0,
                });

            // Calculate military strength
            intel.military_strength = 0.0;
            let mut base_locations = Vec::new();

            for object in game_logic.get_objects().values() {
                if object.team == player.team && object.is_alive() {
                    if object.can_attack() {
                        intel.military_strength += object.health.current * 0.1;
                    }

                    if object.is_kind_of(KindOf::CommandCenter) {
                        base_locations.push(object.get_position());
                    }
                }
            }

            intel.base_locations = base_locations;
            intel.economic_strength = player.resources.supplies as f32;

            // Calculate overall threat
            intel.threat_assessment = intel.military_strength * 0.7 + intel.economic_strength * 0.3;
        }
    }

    /// Update overall threat assessment
    fn update_threat_assessment(&mut self) {
        let total_threat: f32 = self
            .enemy_intel
            .values()
            .map(|intel| intel.threat_assessment)
            .sum();

        self.threat_assessment = match total_threat {
            t if t < 500.0 => ThreatLevel::Minimal,
            t if t < 1000.0 => ThreatLevel::Low,
            t if t < 2000.0 => ThreatLevel::Moderate,
            t if t < 4000.0 => ThreatLevel::High,
            _ => ThreatLevel::Critical,
        };
    }

    /// Manage reconnaissance and scouting
    fn manage_reconnaissance(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Deploy scouts if we don't have enough
        if self.scout_units.len() < 2 {
            self.deploy_scout_unit(game_logic);
        }

        // Update scouted areas
        self.update_scouted_areas(game_logic, current_time);
    }

    /// Deploy a scout unit for reconnaissance
    fn deploy_scout_unit(&mut self, game_logic: &mut GameLogic) {
        // Find a suitable unit to use as scout (fast, expendable)
        let scout_template = match self.base.team {
            Team::USA => "USA_Humvee",
            Team::China => "China_BattleBus",
            Team::GLA => "GLA_Technical",
            _ => return,
        };

        // Try to find an existing unit to reassign
        for (object_id, object) in game_logic.get_objects() {
            if object.team == self.base.team
                && object.is_alive()
                && object.is_mobile()
                && object.get_template().name == scout_template
                && !self.scout_units.contains(object_id)
            {
                self.scout_units.push(*object_id);
                log::debug!(
                    "AI Player {} assigned {} as scout unit",
                    self.base.player_id,
                    object_id
                );
                break;
            }
        }
    }

    /// Update information about scouted areas
    fn update_scouted_areas(&mut self, game_logic: &GameLogic, current_time: f32) {
        // Age existing scouted areas
        for area in &mut self.scouted_areas {
            // Information becomes stale over time
            if current_time - area.last_scouted > 300.0 {
                // 5 minutes
                area.enemy_presence = false; // Assume no longer accurate
            }
        }

        // Update areas around scout units
        for &scout_id in &self.scout_units {
            if let Some(scout) = game_logic.find_object(scout_id) {
                if scout.is_alive() {
                    let scout_pos = scout.get_position();

                    // Check if this area is already scouted recently
                    let mut found_existing = false;
                    let enemy_presence = Self::detect_enemy_presence_static(
                        game_logic,
                        scout_pos,
                        100.0,
                        self.base.team,
                    );

                    for area in &mut self.scouted_areas {
                        if area.center.distance(scout_pos) < area.radius {
                            area.last_scouted = current_time;
                            area.enemy_presence = enemy_presence;
                            found_existing = true;
                            break;
                        }
                    }

                    // Add new scouted area if not found
                    if !found_existing {
                        let strategic_value = Self::calculate_area_strategic_value_static(
                            scout_pos,
                            self.base.base_center,
                            &self.enemy_intel,
                        );

                        self.scouted_areas.push(ScoutedArea {
                            center: scout_pos,
                            radius: 100.0,
                            last_scouted: current_time,
                            enemy_presence,
                            strategic_value,
                        });
                    }
                }
            }
        }
    }

    /// Detect enemy presence in an area
    fn detect_enemy_presence(&self, game_logic: &GameLogic, center: Vec3, radius: f32) -> bool {
        Self::detect_enemy_presence_static(game_logic, center, radius, self.base.team)
    }

    /// Static version to avoid borrowing conflicts
    fn detect_enemy_presence_static(
        game_logic: &GameLogic,
        center: Vec3,
        radius: f32,
        own_team: Team,
    ) -> bool {
        for object in game_logic.get_objects().values() {
            if object.team != own_team
                && object.is_alive()
                && object.get_position().distance(center) <= radius
            {
                return true;
            }
        }
        false
    }

    /// Calculate strategic value of an area
    fn calculate_area_strategic_value(&self, position: Vec3) -> f32 {
        Self::calculate_area_strategic_value_static(
            position,
            self.base.base_center,
            &self.enemy_intel,
        )
    }

    /// Static version to avoid borrowing conflicts
    fn calculate_area_strategic_value_static(
        position: Vec3,
        base_center: Vec3,
        enemy_intel: &HashMap<u32, EnemyIntelligence>,
    ) -> f32 {
        let mut value = 0.0;

        // Value based on distance to our base (closer = more important)
        let distance_to_base = position.distance(base_center);
        value += (300.0 - distance_to_base.min(300.0)) / 300.0 * 50.0;

        // Value based on distance to enemy bases
        for intel in enemy_intel.values() {
            for &enemy_base in &intel.base_locations {
                let distance_to_enemy = position.distance(enemy_base);
                value += (200.0 - distance_to_enemy.min(200.0)) / 200.0 * 30.0;
            }
        }

        value
    }

    /// Update base defense management
    fn update_base_defense_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update threat levels for defensive positions
        self.update_defensive_position_threats(game_logic);

        // Build defensive structures if needed
        self.consider_base_defense_construction(game_logic, current_time);

        // Assign units to defensive positions
        self.assign_defensive_units(game_logic);

        // Check for base defense upgrades
        self.consider_defense_upgrades(game_logic, current_time);
    }

    /// Update threat levels for defensive positions
    fn update_defensive_position_threats(&mut self, game_logic: &GameLogic) {
        for position in &mut self.defensive_positions {
            position.threat_level = 0.0;

            // Check for nearby enemies
            for object in game_logic.get_objects().values() {
                if object.team != self.base.team && object.is_alive() && object.can_attack() {
                    let distance = object.get_position().distance(position.position);
                    if distance < 200.0 {
                        // Threat decreases with distance
                        let threat_contribution = (200.0 - distance) / 200.0;
                        position.threat_level += threat_contribution;
                    }
                }
            }
        }
    }

    /// Consider building defensive structures
    fn consider_base_defense_construction(
        &mut self,
        game_logic: &mut GameLogic,
        _current_time: f32,
    ) {
        // Build defenses based on threat level and available resources
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            if player.resources.supplies >= 1000 {
                // Find position that needs defense most
                let mut highest_threat = 0.0;
                let mut best_position: Option<Vec3> = None;

                for position in &self.defensive_positions {
                    if position.threat_level > highest_threat
                        && position.defensive_structures.is_empty()
                    {
                        highest_threat = position.threat_level;
                        best_position = Some(position.position);
                    }
                }

                if let Some(pos) = best_position {
                    self.build_base_defense_structure(game_logic, pos, false);
                }
            }
        }
    }

    /// Build a defensive structure at specified position
    fn build_base_defense_structure(
        &mut self,
        _game_logic: &mut GameLogic,
        position: Vec3,
        is_flank: bool,
    ) {
        let defense_name = match self.base.team {
            Team::USA => "USA_PatriotMissile",
            Team::China => "USA_GatlingCannon", // China uses same defensive structures in this implementation
            Team::GLA => "GLA_StingerSite",
            _ => return,
        };

        // Add to building queue
        self.base.add_building(defense_name, position, 1);

        if is_flank {
            self.flank_base_defense_count += 1;
        } else {
            self.front_base_defense_count += 1;
        }

        log::debug!(
            "AI Player {} building {} defense at {:?}",
            self.base.player_id,
            defense_name,
            position
        );
    }

    /// Assign units to defensive positions
    fn assign_defensive_units(&mut self, game_logic: &GameLogic) {
        // Find units that can be used for defense
        let mut available_defenders = Vec::new();

        for (object_id, object) in game_logic.get_objects() {
            if object.team == self.base.team
                && object.is_alive()
                && object.can_attack()
                && !object.status.moving
                && !self.is_unit_in_attack_group(*object_id)
            {
                available_defenders.push(*object_id);
            }
        }

        // Assign defenders to positions based on threat level
        let mut defender_index = 0;
        for position in &mut self.defensive_positions {
            if position.defending_units.len() < 2 && defender_index < available_defenders.len() {
                let defender_id = available_defenders[defender_index];
                position.defending_units.push(defender_id);
                self.base.defensive_units.push(defender_id);
                defender_index += 1;

                // Command unit to move to defensive position
                if let Some(_defender) = game_logic.find_object(defender_id) {
                    // In a real implementation, we would command the unit to move
                    log::debug!(
                        "AI Player {} assigning unit {} to defend position {:?}",
                        self.base.player_id,
                        defender_id,
                        position.position
                    );
                }
            }
        }
    }

    /// Check if unit is part of an attack group
    fn is_unit_in_attack_group(&self, unit_id: ObjectId) -> bool {
        for group in &self.attack_groups {
            if group.units.contains(&unit_id) {
                return true;
            }
        }
        false
    }

    /// Consider defense upgrades and improvements
    fn consider_defense_upgrades(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Check if we should upgrade existing defenses
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            if player.resources.supplies >= 2000 {
                // Look for opportunities to upgrade defensive structures
                // This would be implemented with specific upgrade logic
            }
        }
    }

    /// Update attack coordination and group management
    fn update_attack_coordination(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update existing attack groups
        self.update_attack_groups(game_logic, current_time);

        // Consider forming new attack groups
        self.consider_new_attack_groups(game_logic, current_time);

        // Coordinate multi-group attacks
        self.coordinate_combined_attacks(game_logic, current_time);
    }

    /// Update status of existing attack groups
    fn update_attack_groups(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        let mut groups_to_remove = Vec::new();

        for (i, group) in self.attack_groups.iter_mut().enumerate() {
            // Remove dead units from group
            group.units.retain(|&unit_id| {
                if let Some(unit) = game_logic.find_object(unit_id) {
                    unit.is_alive()
                } else {
                    false
                }
            });

            // Disband group if too few units remain
            if group.units.len() < 2 {
                group.status = GroupStatus::Disbanded;
                groups_to_remove.push(i);
                continue;
            }

            // Update group status based on situation
            Self::update_group_status_static(group, game_logic, current_time);
        }

        // Remove disbanded groups
        for &index in groups_to_remove.iter().rev() {
            let group = self.attack_groups.remove(index);
            log::debug!(
                "AI Player {} disbanded attack group: {}",
                self.base.player_id,
                group.name
            );
        }
    }

    /// Update individual group status
    fn update_group_status(
        &self,
        group: &mut AttackGroup,
        game_logic: &GameLogic,
        current_time: f32,
    ) {
        Self::update_group_status_static(group, game_logic, current_time);
    }

    /// Static version to avoid borrowing conflicts
    fn update_group_status_static(
        group: &mut AttackGroup,
        game_logic: &GameLogic,
        _current_time: f32,
    ) {
        // Check if group is in combat
        let mut in_combat = false;
        let mut at_objective = true;

        for &unit_id in &group.units {
            if let Some(unit) = game_logic.find_object(unit_id) {
                // Check if unit is fighting
                if unit.status.attacking || unit.target.is_some() {
                    in_combat = true;
                }

                // Check if unit is near objective
                if unit.get_position().distance(group.target_position) > 50.0 {
                    at_objective = false;
                }
            }
        }

        // Update status
        group.status = match (in_combat, at_objective) {
            (true, _) => GroupStatus::Engaging,
            (false, true) => GroupStatus::Moving,
            (false, false) => GroupStatus::Moving,
        };
    }

    /// Consider forming new attack groups
    fn consider_new_attack_groups(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Don't form new groups too frequently
        if current_time - self.base.last_attack_time < 120.0 {
            return;
        }

        // Check if we have enough idle military units
        let idle_units = self.find_idle_military_units(game_logic);

        if idle_units.len() >= 4 {
            let group_name = format!("Attack_{}", current_time as u32);
            let target = self.select_attack_target(game_logic);

            let attack_group = AttackGroup {
                name: group_name.clone(),
                units: idle_units.into_iter().take(6).collect(), // Take up to 6 units
                objective: AttackObjective::DestroyBase,
                formation: Formation::Wedge,
                status: GroupStatus::Forming,
                rally_point: self.base.base_center + Vec3::new(0.0, 0.0, -100.0),
                target_position: target,
            };

            self.attack_groups.push(attack_group);
            log::debug!(
                "AI Player {} formed new attack group: {}",
                self.base.player_id,
                group_name
            );
        }
    }

    /// Find idle military units available for attack groups
    fn find_idle_military_units(&self, game_logic: &GameLogic) -> Vec<ObjectId> {
        let mut idle_units = Vec::new();

        for (object_id, object) in game_logic.get_objects() {
            if object.team == self.base.team
                && object.is_alive()
                && object.can_attack()
                && object.is_mobile()
                && !object.status.moving
                && !object.status.attacking
                && !self.is_unit_in_attack_group(*object_id)
                && !self.base.defensive_units.contains(object_id)
            {
                idle_units.push(*object_id);
            }
        }

        idle_units
    }

    /// Select target for attack
    fn select_attack_target(&self, game_logic: &GameLogic) -> Vec3 {
        // Prioritize enemy command centers
        for intel in self.enemy_intel.values() {
            if !intel.base_locations.is_empty() {
                return intel.base_locations[0];
            }
        }

        // Fallback to enemy units
        for object in game_logic.get_objects().values() {
            if object.team != self.base.team && object.is_alive() {
                return object.get_position();
            }
        }

        // Default target
        -self.base.base_center
    }

    /// Coordinate multiple attack groups for combined operations
    fn coordinate_combined_attacks(&mut self, _game_logic: &GameLogic, _current_time: f32) {
        // If we have multiple attack groups, coordinate their timing
        if self.attack_groups.len() >= 2 {
            // Check if groups should attack simultaneously
            let ready_groups: Vec<_> = self
                .attack_groups
                .iter()
                .filter(|g| g.status == GroupStatus::Moving && g.units.len() >= 3)
                .collect();

            if ready_groups.len() >= 2 {
                // Coordinate timing - wait for all groups to be in position
                log::debug!(
                    "AI Player {} coordinating combined attack with {} groups",
                    self.base.player_id,
                    ready_groups.len()
                );
            }
        }
    }

    /// Update economic optimization and resource management
    fn update_economic_optimization(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Analyze economic efficiency
        self.analyze_economic_performance(game_logic);

        // Consider expansion opportunities
        self.evaluate_expansion_sites(game_logic, current_time);

        // Optimize resource allocation
        self.optimize_resource_allocation(game_logic);

        // Manage supply routes and security
        self.manage_supply_route_security(game_logic);
    }

    /// Analyze current economic performance
    fn analyze_economic_performance(&mut self, game_logic: &GameLogic) {
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            let supply_income = self.calculate_supply_income(game_logic);
            let supply_consumption = self.calculate_supply_consumption(game_logic);

            // Adjust economic focus based on performance
            self.economic_focus = match (
                player.resources.supplies,
                supply_income - supply_consumption,
            ) {
                (supplies, income) if supplies < 500 || income < 0 => EconomicFocus::SupplyRush,
                (supplies, _) if supplies > 3000 => EconomicFocus::MassProduction,
                _ => EconomicFocus::Balanced,
            };
        }
    }

    /// Calculate current supply income
    fn calculate_supply_income(&self, game_logic: &GameLogic) -> i32 {
        let mut income = 0;

        for object in game_logic.get_objects().values() {
            if object.team == self.base.team
                && object.is_alive()
                && object.is_constructed()
                && object.is_kind_of(KindOf::SupplyCenter)
            {
                income += 2; // Base income per supply center
            }
        }

        income
    }

    /// Calculate supply consumption rate
    fn calculate_supply_consumption(&self, game_logic: &GameLogic) -> i32 {
        let mut consumption = 0;

        // Count units that consume supplies
        for object in game_logic.get_objects().values() {
            if object.team == self.base.team && object.is_alive() {
                if object.is_kind_of(KindOf::Infantry) {
                    consumption += 1;
                } else if object.is_kind_of(KindOf::Vehicle) {
                    consumption += 2;
                } else if object.is_kind_of(KindOf::Aircraft) {
                    consumption += 3;
                }
            }
        }

        consumption
    }

    /// Evaluate potential expansion sites
    fn evaluate_expansion_sites(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Collect sites that need expansion consideration
        let mut sites_to_expand = Vec::new();

        for (i, site) in self.expansion_sites.iter_mut().enumerate() {
            // Update security level based on recent enemy activity
            site.security_level =
                Self::calculate_site_security_static(game_logic, site.position, self.base.team);

            // Consider expanding to secure sites with good resources
            if !site.is_occupied && site.security_level > 70.0 && site.resource_value > 80.0 {
                sites_to_expand.push(i);
            }
        }

        // Process expansion for collected sites
        for site_index in sites_to_expand {
            if let Some(_site) = self.expansion_sites.get_mut(site_index) {
                self.consider_expansion_to_site_by_index(game_logic, site_index);
            }
        }
    }

    /// Calculate security level for an expansion site
    fn calculate_site_security(&self, game_logic: &GameLogic, position: Vec3) -> f32 {
        Self::calculate_site_security_static(game_logic, position, self.base.team)
    }

    /// Static version to avoid borrowing conflicts
    fn calculate_site_security_static(
        game_logic: &GameLogic,
        position: Vec3,
        own_team: Team,
    ) -> f32 {
        let mut security = 100.0;

        // Reduce security based on nearby enemies
        for object in game_logic.get_objects().values() {
            if object.team != own_team && object.is_alive() && object.can_attack() {
                let distance = object.get_position().distance(position);
                if distance < 300.0 {
                    security -= (300.0 - distance) / 300.0 * 20.0;
                }
            }
        }

        security.max(0.0)
    }

    /// Consider expanding to a specific site
    fn consider_expansion_to_site(&mut self, game_logic: &GameLogic, site: &mut ExpansionSite) {
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            if player.resources.supplies >= 1500 {
                // Mark site as occupied and plan expansion
                site.is_occupied = true;

                // Add expansion buildings to queue
                let command_center = match self.base.team {
                    Team::USA => "USA_CommandCenter",
                    Team::China => "China_CommandCenter",
                    Team::GLA => "GLA_CommandCenter",
                    _ => return,
                };

                self.base.add_building(command_center, site.position, 1);

                log::debug!(
                    "AI Player {} planning expansion at {:?}",
                    self.base.player_id,
                    site.position
                );
            }
        }
    }

    /// Consider expansion to site by index to avoid borrowing conflicts
    fn consider_expansion_to_site_by_index(&mut self, game_logic: &GameLogic, site_index: usize) {
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            if player.resources.supplies >= 1500 {
                if let Some(site) = self.expansion_sites.get_mut(site_index) {
                    // Mark site as occupied and plan expansion
                    site.is_occupied = true;

                    // Add expansion buildings to queue
                    let command_center = match self.base.team {
                        Team::USA => "USA_CommandCenter",
                        Team::China => "China_CommandCenter",
                        Team::GLA => "GLA_CommandCenter",
                        _ => return,
                    };

                    self.base.add_building(command_center, site.position, 1);

                    log::debug!(
                        "AI Player {} planning expansion at {:?}",
                        self.base.player_id,
                        site.position
                    );
                }
            }
        }
    }

    /// Optimize resource allocation between military and economy
    fn optimize_resource_allocation(&mut self, game_logic: &GameLogic) {
        if let Some(_player) = game_logic.get_player(self.base.player_id) {
            let military_units = self.count_military_units(game_logic);
            let economic_structures = self.count_economic_structures(game_logic);

            // Adjust build priorities based on current balance
            match (military_units, economic_structures) {
                (_m, e) if e < 3 => {
                    // Focus on economy first
                    self.base.build_phase = AIBuildPhase::BaseConstruction;
                }
                (m, _e) if m < 5 => {
                    // Build initial military
                    self.base.build_phase = AIBuildPhase::UnitProduction;
                }
                _ => {
                    // Balanced growth
                    self.base.build_phase = AIBuildPhase::MassProduction;
                }
            }
        }
    }

    /// Count military units
    fn count_military_units(&self, game_logic: &GameLogic) -> u32 {
        let mut count = 0;
        for object in game_logic.get_objects().values() {
            if object.team == self.base.team && object.is_alive() && object.can_attack() {
                count += 1;
            }
        }
        count
    }

    /// Count economic structures
    fn count_economic_structures(&self, game_logic: &GameLogic) -> u32 {
        let mut count = 0;
        for object in game_logic.get_objects().values() {
            if object.team == self.base.team
                && object.is_alive()
                && (object.is_kind_of(KindOf::SupplyCenter)
                    || object.is_kind_of(KindOf::PowerPlant))
            {
                count += 1;
            }
        }
        count
    }

    /// Manage supply route security
    fn manage_supply_route_security(&mut self, game_logic: &GameLogic) {
        // Collect positions that need protection
        let mut positions_needing_protection = Vec::new();

        // Identify critical supply routes that need protection
        for site in &self.expansion_sites {
            if site.is_occupied {
                // Check if route to expansion needs guards
                let distance = site.position.distance(self.base.base_center);
                if distance > 200.0 {
                    positions_needing_protection.push(site.position);
                }
            }
        }

        // Apply protection to collected positions
        for position in positions_needing_protection {
            self.consider_supply_route_protection(game_logic, position);
        }
    }

    /// Consider protecting a supply route
    fn consider_supply_route_protection(&mut self, game_logic: &GameLogic, destination: Vec3) {
        // Find midpoint of route
        let midpoint = (self.base.base_center + destination) / 2.0;

        // Check if area needs protection
        if self.detect_enemy_presence(game_logic, midpoint, 150.0) {
            // Assign patrol units if available
            let patrol_units = self.find_idle_military_units(game_logic);

            if patrol_units.len() >= 2 {
                // Create patrol route
                for &unit_id in patrol_units.iter().take(2) {
                    self.supply_route_security.push(unit_id);

                    // Command unit to patrol area (in real implementation)
                    log::debug!(
                        "AI Player {} assigning unit {} to patrol supply route",
                        self.base.player_id,
                        unit_id
                    );
                }
            }
        }
    }

    /// Update superweapon management and deployment
    fn update_superweapon_management(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Check superweapon availability
        self.check_superweapon_availability(game_logic);

        // Consider using available superweapons
        if current_time - self.last_superweapon_use > 300.0 {
            // 5 minute cooldown
            self.evaluate_superweapon_targets(game_logic, current_time);
        }
    }

    /// Check which superweapons are available
    fn check_superweapon_availability(&mut self, game_logic: &GameLogic) {
        // In a full implementation, this would check actual superweapon structures
        // and their readiness status
        if let Some(player) = game_logic.get_player(self.base.player_id) {
            // Simple check: if we have enough resources, assume superweapons are available
            if player.resources.supplies >= 5000 {
                // Superweapons are available
            }
        }
    }

    /// Evaluate targets for superweapon deployment
    fn evaluate_superweapon_targets(&mut self, game_logic: &GameLogic, current_time: f32) {
        let mut best_targets = Vec::new();

        // Find high-value enemy targets
        for intel in self.enemy_intel.values() {
            for &base_location in &intel.base_locations {
                let target_value =
                    self.calculate_superweapon_target_value(game_logic, base_location);
                if target_value > 100.0 {
                    best_targets.push((base_location, target_value));
                }
            }
        }

        // Sort by value and use best target
        if !best_targets.is_empty() {
            best_targets.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let (target_pos, _) = best_targets[0];

            self.deploy_superweapon(game_logic, target_pos, current_time);
        }
    }

    /// Calculate value of a potential superweapon target
    fn calculate_superweapon_target_value(&self, game_logic: &GameLogic, position: Vec3) -> f32 {
        let mut value = 0.0;
        let range = 100.0; // Superweapon effective radius

        for object in game_logic.get_objects().values() {
            if object.team != self.base.team
                && object.is_alive()
                && object.get_position().distance(position) <= range
            {
                // Value based on object type
                if object.is_kind_of(KindOf::CommandCenter) {
                    value += 200.0;
                } else if object.is_kind_of(KindOf::Structure) {
                    value += 50.0;
                } else if object.can_attack() {
                    value += 30.0;
                } else {
                    value += 10.0;
                }
            }
        }

        value
    }

    /// Deploy superweapon at target location
    fn deploy_superweapon(&mut self, _game_logic: &GameLogic, target: Vec3, current_time: f32) {
        if !self.available_superweapons.is_empty() {
            let weapon = &self.available_superweapons[0];

            log::debug!(
                "AI Player {} deploying {} at {:?}",
                self.base.player_id,
                weapon,
                target
            );

            // In a real implementation, this would trigger the actual superweapon
            self.last_superweapon_use = current_time;
            self.superweapon_targets.push(target);
        }
    }

    /// Update tactical combat management
    fn update_tactical_combat(&mut self, game_logic: &mut GameLogic, current_time: f32) {
        // Update combat groups
        self.update_combat_groups(game_logic, current_time);

        // Manage unit veterancy and promotions
        self.manage_veterancy_system(game_logic);

        // Handle retreats and regrouping
        self.handle_tactical_retreats(game_logic, current_time);
    }

    /// Update combat group effectiveness and composition
    fn update_combat_groups(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Collect group updates to avoid borrow conflicts
        let mut group_updates = Vec::new();

        for (group_name, group) in &mut self.combat_groups {
            // Remove dead units
            group.units.retain(|&unit_id| {
                if let Some(unit) = game_logic.find_object(unit_id) {
                    unit.is_alive()
                } else {
                    false
                }
            });

            // Calculate effectiveness for this group
            if !group.units.is_empty() {
                let effectiveness =
                    Self::calculate_group_effectiveness_static(game_logic, &group.units);
                group_updates.push((group_name.clone(), effectiveness));
            }
        }

        // Apply updates
        for (group_name, effectiveness) in group_updates {
            if let Some(group) = self.combat_groups.get_mut(&group_name) {
                group.effectiveness = effectiveness;
            }
        }
    }

    /// Calculate combat group effectiveness
    fn calculate_group_effectiveness(&self, game_logic: &GameLogic, group: &CombatGroup) -> f32 {
        Self::calculate_group_effectiveness_static(game_logic, &group.units)
    }

    /// Static version to avoid borrowing conflicts
    fn calculate_group_effectiveness_static(game_logic: &GameLogic, units: &[ObjectId]) -> f32 {
        let mut effectiveness = 0.0;
        let mut unit_count = 0;

        for &unit_id in units {
            if let Some(unit) = game_logic.find_object(unit_id) {
                // Factor in health, veterancy, and weapon effectiveness
                let health_factor = unit.health.percentage();
                let veterancy_factor = match unit.experience.level {
                    VeterancyLevel::Rookie => 1.0,
                    VeterancyLevel::Veteran => 1.2,
                    VeterancyLevel::Elite => 1.5,
                    VeterancyLevel::Heroic => 2.0,
                };

                effectiveness += health_factor * veterancy_factor;
                unit_count += 1;
            }
        }

        if unit_count > 0 {
            effectiveness / unit_count as f32
        } else {
            0.0
        }
    }

    /// Manage unit veterancy and experience system
    fn manage_veterancy_system(&mut self, game_logic: &GameLogic) {
        // Promote experienced units to special roles
        for object in game_logic.get_objects().values() {
            if object.team == self.base.team
                && object.is_alive()
                && object.experience.level != VeterancyLevel::Rookie
            {
                // Consider special assignments for veteran units
                if object.experience.level == VeterancyLevel::Elite
                    || object.experience.level == VeterancyLevel::Heroic
                {
                    // Assign to elite combat group or special operations

                    if !self.combat_groups.contains_key("Elite") {
                        self.combat_groups.insert(
                            "Elite".to_string(),
                            CombatGroup {
                                units: Vec::new(),
                                role: CombatRole::Elite,
                                experience_level: 2.0,
                                morale: 1.0,
                                effectiveness: 1.5,
                            },
                        );
                    }

                    if let Some(elite_group) = self.combat_groups.get_mut("Elite") {
                        if !elite_group.units.contains(&object.id) {
                            elite_group.units.push(object.id);
                        }
                    }
                }
            }
        }
    }

    /// Handle tactical retreats when outmatched
    fn handle_tactical_retreats(&mut self, game_logic: &GameLogic, _current_time: f32) {
        // Collect retreat decisions to avoid borrow conflicts
        let mut retreat_decisions = Vec::new();

        // Check if any attack groups should retreat
        for (i, group) in self.attack_groups.iter().enumerate() {
            if group.status == GroupStatus::Engaging {
                let group_health =
                    Self::calculate_group_total_health_static(game_logic, &group.units);
                let nearby_enemy_strength = Self::calculate_nearby_enemy_strength_static(
                    game_logic,
                    group.target_position,
                    self.base.team,
                );

                // Retreat if severely outmatched
                if group_health < nearby_enemy_strength * 0.3 {
                    let retreat_pos = Self::find_nearest_retreat_position_static(
                        &self.retreat_positions,
                        self.base.base_center,
                        group.target_position,
                    );
                    retreat_decisions.push((i, retreat_pos));
                }
            }
        }

        // Apply retreat decisions
        for (group_index, retreat_pos) in retreat_decisions {
            if let Some(group) = self.attack_groups.get_mut(group_index) {
                group.status = GroupStatus::Retreating;
                group.target_position = retreat_pos;

                log::debug!(
                    "AI Player {} ordering retreat for group {}",
                    self.base.player_id,
                    group.name
                );
            }
        }
    }

    /// Calculate total health of a group of units
    fn calculate_group_total_health(&self, game_logic: &GameLogic, units: &[ObjectId]) -> f32 {
        Self::calculate_group_total_health_static(game_logic, units)
    }

    /// Static version to avoid borrowing conflicts
    fn calculate_group_total_health_static(game_logic: &GameLogic, units: &[ObjectId]) -> f32 {
        let mut total_health = 0.0;

        for &unit_id in units {
            if let Some(unit) = game_logic.find_object(unit_id) {
                total_health += unit.health.current;
            }
        }

        total_health
    }

    /// Calculate enemy strength near a position
    fn calculate_nearby_enemy_strength(&self, game_logic: &GameLogic, position: Vec3) -> f32 {
        Self::calculate_nearby_enemy_strength_static(game_logic, position, self.base.team)
    }

    /// Static version to avoid borrowing conflicts
    fn calculate_nearby_enemy_strength_static(
        game_logic: &GameLogic,
        position: Vec3,
        own_team: Team,
    ) -> f32 {
        let mut strength = 0.0;

        for object in game_logic.get_objects().values() {
            if object.team != own_team
                && object.is_alive()
                && object.can_attack()
                && object.get_position().distance(position) < 200.0
            {
                strength += object.health.current;
            }
        }

        strength
    }

    /// Find the nearest retreat position to a given location
    fn find_nearest_retreat_position(&self, from_position: Vec3) -> Vec3 {
        Self::find_nearest_retreat_position_static(
            &self.retreat_positions,
            self.base.base_center,
            from_position,
        )
    }

    /// Static version to avoid borrowing conflicts
    fn find_nearest_retreat_position_static(
        retreat_positions: &[Vec3],
        base_center: Vec3,
        from_position: Vec3,
    ) -> Vec3 {
        let mut nearest_pos = base_center;
        let mut nearest_distance = from_position.distance(base_center);

        for &retreat_pos in retreat_positions {
            let distance = from_position.distance(retreat_pos);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_pos = retreat_pos;
            }
        }

        nearest_pos
    }

    /// Get comprehensive AI status information
    pub fn get_status_info(&self) -> String {
        format!(
            "AI Skirmish Player {} ({}):\n\
             Strategy: {:?} | Build Phase: {:?} | Threat: {:?}\n\
             Military: {} attack groups, {} scouts\n\
             Defense: {} front, {} flank defenses\n\
             Economy: {:?} focus, {} expansion sites\n\
             Intel: {} enemies tracked, {} areas scouted",
            self.base.player_id,
            self.base.team.get_name(),
            self.base.current_strategy,
            self.base.build_phase,
            self.threat_assessment,
            self.attack_groups.len(),
            self.scout_units.len(),
            self.front_base_defense_count,
            self.flank_base_defense_count,
            self.economic_focus,
            self.expansion_sites.len(),
            self.enemy_intel.len(),
            self.scouted_areas.len()
        )
    }
}
