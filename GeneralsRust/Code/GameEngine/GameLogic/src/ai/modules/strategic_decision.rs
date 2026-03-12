//! Strategic Decision Making System
//!
//! Manages high-level strategic decisions for AI players including:
//! - When to attack vs defend
//! - When to expand vs consolidate
//! - Resource investment priorities
//! - Tech progression vs military buildup
//!
//! Ported from C++ AIPlayer.cpp and AISkirmishPlayer.cpp
//! Matches C++ behavior for game balance

use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::HashMap;

/// Strategic decision types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategicDecision {
    /// Build up military forces
    BuildUpForces,
    /// Launch attack on enemy
    LaunchAttack,
    /// Defend base
    DefendBase,
    /// Expand territory
    Expand,
    /// Focus on economy
    EconomicGrowth,
    /// Advance technology
    TechProgression,
    /// Harass enemy economy
    Harassment,
    /// Turtle and build defenses
    Turtle,
    /// All-out attack
    AllOut,
}

/// Strategic stance of AI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategicStance {
    /// Very defensive, turtling
    VeryDefensive,
    /// Moderately defensive
    Defensive,
    /// Balanced approach
    Balanced,
    /// Moderately aggressive
    Aggressive,
    /// All-out aggression
    VeryAggressive,
}

/// Game phase for strategy adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GamePhase {
    /// Early game (0-5 minutes)
    Early,
    /// Mid game (5-15 minutes)
    Mid,
    /// Late game (15+ minutes)
    Late,
}

/// Attack timing criteria - matches C++ AIPlayer logic
#[derive(Debug, Clone)]
pub struct AttackTiming {
    /// Minimum military strength before attacking (0.0 to 1.0)
    pub min_military_strength: f32,
    /// Minimum army size before attacking
    pub min_army_size: i32,
    /// Frames since last attack
    pub frames_since_last_attack: u32,
    /// Minimum frames between attacks
    pub min_attack_interval: u32,
    /// Enemy strength estimate (0.0 to 1.0)
    pub enemy_strength: f32,
    /// Confidence in attack success (0.0 to 1.0)
    pub attack_confidence: f32,
}

impl AttackTiming {
    pub fn new() -> Self {
        Self {
            min_military_strength: 0.6,
            min_army_size: 10,
            frames_since_last_attack: 0,
            // Matches C++ teamSeconds default of 60 frames (2 seconds at 30 FPS)
            min_attack_interval: 300, // 10 seconds at 30 FPS
            enemy_strength: 0.5,
            attack_confidence: 0.5,
        }
    }

    /// Check if we're ready to attack - matches C++ selectTeamToBuild logic
    pub fn is_ready_to_attack(&self) -> bool {
        // Must have minimum strength
        if self.min_military_strength < 0.5 {
            return false;
        }

        // Must have minimum army size
        if self.min_army_size < 5 {
            return false;
        }

        // Must wait minimum interval
        if self.frames_since_last_attack < self.min_attack_interval {
            return false;
        }

        // Confidence check - need at least 60% confidence
        if self.attack_confidence < 0.6 {
            return false;
        }

        // Compare our strength to enemy - need advantage or near-parity
        if self.min_military_strength < self.enemy_strength * 0.8 {
            return false;
        }

        true
    }

    /// Calculate attack confidence based on situation
    pub fn calculate_attack_confidence(
        &mut self,
        our_strength: f32,
        enemy_strength: f32,
        our_base_health: f32,
        enemy_threat_level: f32,
    ) {
        self.enemy_strength = enemy_strength;

        // Base confidence on strength ratio
        let strength_ratio = if enemy_strength > 0.0 {
            our_strength / enemy_strength
        } else {
            2.0 // No enemy detected = high confidence
        };

        // Start with strength ratio
        let mut confidence = strength_ratio.min(1.0);

        // Penalize if our base is damaged
        confidence *= our_base_health;

        // Penalize if enemy threat is high
        confidence *= 1.0 - enemy_threat_level * 0.5;

        // Clamp to valid range
        self.attack_confidence = confidence.max(0.0).min(1.0);
    }
}

impl Default for AttackTiming {
    fn default() -> Self {
        Self::new()
    }
}

/// Expansion strategy - matches C++ expansion logic
#[derive(Debug, Clone)]
pub struct ExpansionStrategy {
    /// Is expansion currently viable
    pub can_expand: bool,
    /// Number of expansions taken
    pub expansion_count: i32,
    /// Maximum expansions allowed
    pub max_expansions: i32,
    /// Minimum resources needed for expansion
    pub min_resources_for_expansion: i32,
    /// Frames since last expansion
    pub frames_since_expansion: u32,
    /// Minimum frames between expansions
    pub min_expansion_interval: u32,
}

impl ExpansionStrategy {
    pub fn new() -> Self {
        Self {
            can_expand: false,
            expansion_count: 0,
            max_expansions: 2,
            min_resources_for_expansion: 2000, // Matches C++ resource thresholds
            frames_since_expansion: 0,
            min_expansion_interval: 1800, // 60 seconds at 30 FPS
        }
    }

    /// Check if we should expand - matches C++ expansion logic in AIPlayer.cpp
    pub fn should_expand(
        &self,
        current_resources: i32,
        enemy_distance: f32,
        base_security: f32,
    ) -> bool {
        // Can't expand if at limit
        if self.expansion_count >= self.max_expansions {
            return false;
        }

        // Need minimum resources
        if current_resources < self.min_resources_for_expansion {
            return false;
        }

        // Must wait minimum interval (but allow first expansion immediately)
        if self.expansion_count > 0 && self.frames_since_expansion < self.min_expansion_interval {
            return false;
        }

        // Enemy must not be too close (from C++ AIPlayer.cpp line ~2800)
        // "closer than 60/40 to enemy than to us, probably not a good candidate for expansion"
        if enemy_distance < 400.0 {
            return false;
        }

        // Base must be relatively secure
        if base_security < 0.7 {
            return false;
        }

        true
    }

    /// Update expansion state
    pub fn on_expansion_complete(&mut self) {
        self.expansion_count += 1;
        self.frames_since_expansion = 0;
    }
}

impl Default for ExpansionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource management strategy - matches C++ resource logic
#[derive(Debug, Clone)]
pub struct ResourceManagementStrategy {
    /// Current total resources
    pub current_resources: i32,
    /// Resources per second income rate
    pub income_rate: f32,
    /// Resource allocation by category
    pub allocations: HashMap<String, f32>,
    /// Is economy under pressure
    pub economic_pressure: bool,
    /// Are we wealthy (matches C++ m_resourcesWealthy)
    pub is_wealthy: bool,
    /// Are we poor (matches C++ m_resourcesPoor)
    pub is_poor: bool,
}

impl ResourceManagementStrategy {
    pub fn new() -> Self {
        let mut allocations = HashMap::new();
        allocations.insert("military".to_string(), 0.5);
        allocations.insert("economy".to_string(), 0.3);
        allocations.insert("defense".to_string(), 0.1);
        allocations.insert("tech".to_string(), 0.1);

        Self {
            current_resources: 0,
            income_rate: 0.0,
            allocations,
            economic_pressure: false,
            is_wealthy: false,
            is_poor: false,
        }
    }

    /// Update resource state - matches C++ AIData thresholds
    pub fn update(&mut self, resources: i32, ai_data_wealthy: i32, ai_data_poor: i32) {
        self.current_resources = resources;

        // Matches C++ AIData default values
        // m_resourcesWealthy and m_resourcesPoor from AI.cpp
        self.is_wealthy = resources > ai_data_wealthy;
        self.is_poor = resources < ai_data_poor;

        self.economic_pressure = self.is_poor;
    }

    /// Get allocation for specific category
    pub fn get_allocation(&self, category: &str) -> f32 {
        *self.allocations.get(category).unwrap_or(&0.0)
    }

    /// Adjust allocations based on strategy
    pub fn adjust_for_strategy(&mut self, stance: StrategicStance, phase: GamePhase) {
        // Clear and rebuild allocations
        self.allocations.clear();

        match (stance, phase) {
            // Early game aggressive - focus military
            (StrategicStance::VeryAggressive, GamePhase::Early) => {
                self.allocations.insert("military".to_string(), 0.7);
                self.allocations.insert("economy".to_string(), 0.2);
                self.allocations.insert("defense".to_string(), 0.05);
                self.allocations.insert("tech".to_string(), 0.05);
            }
            // Defensive early game - economy and defense
            (StrategicStance::VeryDefensive, GamePhase::Early) => {
                self.allocations.insert("military".to_string(), 0.2);
                self.allocations.insert("economy".to_string(), 0.4);
                self.allocations.insert("defense".to_string(), 0.3);
                self.allocations.insert("tech".to_string(), 0.1);
            }
            // Balanced approach
            (StrategicStance::Balanced, _) => {
                self.allocations.insert("military".to_string(), 0.4);
                self.allocations.insert("economy".to_string(), 0.3);
                self.allocations.insert("defense".to_string(), 0.2);
                self.allocations.insert("tech".to_string(), 0.1);
            }
            // Late game aggressive - all-in on military
            (StrategicStance::VeryAggressive, GamePhase::Late) => {
                self.allocations.insert("military".to_string(), 0.8);
                self.allocations.insert("economy".to_string(), 0.1);
                self.allocations.insert("defense".to_string(), 0.05);
                self.allocations.insert("tech".to_string(), 0.05);
            }
            // Default case
            _ => {
                self.allocations.insert("military".to_string(), 0.5);
                self.allocations.insert("economy".to_string(), 0.3);
                self.allocations.insert("defense".to_string(), 0.1);
                self.allocations.insert("tech".to_string(), 0.1);
            }
        }
    }

    /// Calculate build speed modifier - matches C++ structureTimer logic
    pub fn get_build_speed_modifier(&self) -> f32 {
        // From C++ AIPlayer.cpp and AISkirmishPlayer.cpp
        // If wealthy, build faster (divide timer by wealthyMod)
        // If poor, build slower (divide timer by poorMod)
        if self.is_wealthy {
            1.5 // Build 50% faster
        } else if self.is_poor {
            0.7 // Build 30% slower
        } else {
            1.0
        }
    }
}

impl Default for ResourceManagementStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// Main strategic decision-making system
#[derive(Debug)]
pub struct StrategicDecisionMaker {
    /// Current strategic stance
    pub current_stance: StrategicStance,
    /// Current game phase
    pub current_phase: GamePhase,
    /// Attack timing system
    pub attack_timing: AttackTiming,
    /// Expansion strategy
    pub expansion: ExpansionStrategy,
    /// Resource management
    pub resources: ResourceManagementStrategy,
    /// Last decision made
    pub last_decision: Option<StrategicDecision>,
    /// Frames since last decision
    pub frames_since_decision: u32,
    /// Confidence in current strategy (0.0 to 1.0)
    pub strategy_confidence: f32,
    /// Difficulty factor for AI scaling (1.0 = normal)
    pub difficulty_factor: f32,
}

impl StrategicDecisionMaker {
    pub fn new() -> Self {
        Self {
            current_stance: StrategicStance::Balanced,
            current_phase: GamePhase::Early,
            attack_timing: AttackTiming::new(),
            expansion: ExpansionStrategy::new(),
            resources: ResourceManagementStrategy::new(),
            last_decision: None,
            frames_since_decision: 0,
            strategy_confidence: 1.0,
            difficulty_factor: 1.0,
        }
    }

    /// Main decision-making update - matches C++ AIPlayer::update logic
    pub fn update(&mut self, current_frame: u32) {
        // Update game phase based on frame count (30 FPS assumed)
        self.current_phase = if current_frame < 9000 {
            GamePhase::Early // 0-5 minutes
        } else if current_frame < 27000 {
            GamePhase::Mid // 5-15 minutes
        } else {
            GamePhase::Late // 15+ minutes
        };

        // Update timers
        self.attack_timing.frames_since_last_attack += 1;
        self.expansion.frames_since_expansion += 1;
        self.frames_since_decision += 1;

        // Adjust resource allocations based on current situation
        self.resources
            .adjust_for_strategy(self.current_stance, self.current_phase);
    }

    /// Make strategic decision based on current situation
    pub fn make_decision(
        &mut self,
        military_strength: f32,
        enemy_strength: f32,
        base_health: f32,
        threat_level: f32,
        available_resources: i32,
    ) -> StrategicDecision {
        // Cache the current strengths so AttackTiming heuristics work off live data.
        self.attack_timing.min_military_strength = military_strength;
        self.attack_timing.enemy_strength = enemy_strength;

        // Update attack confidence
        self.attack_timing.calculate_attack_confidence(
            military_strength,
            enemy_strength,
            base_health,
            threat_level,
        );

        // Determine decision based on situation
        let decision = if threat_level > 0.7 {
            // High threat - defend
            StrategicDecision::DefendBase
        } else if self.resources.economic_pressure {
            // Low resources - focus economy
            StrategicDecision::EconomicGrowth
        } else if self.attack_timing.is_ready_to_attack()
            && self.current_stance != StrategicStance::VeryDefensive
        {
            // Ready to attack
            StrategicDecision::LaunchAttack
        } else if self.expansion.can_expand
            && self
                .expansion
                .should_expand(available_resources, 500.0, base_health)
        {
            // Good time to expand
            StrategicDecision::Expand
        } else if military_strength < 0.5 {
            // Need more military
            StrategicDecision::BuildUpForces
        } else {
            // Default to building up forces (safer fallback than spending purely on economy).
            StrategicDecision::BuildUpForces
        };

        self.last_decision = Some(decision);
        self.frames_since_decision = 0;

        decision
    }

    /// Set strategic stance manually
    pub fn set_stance(&mut self, stance: StrategicStance) {
        self.current_stance = stance;
        self.resources
            .adjust_for_strategy(stance, self.current_phase);
    }

    /// Called when attack is launched
    pub fn on_attack_launched(&mut self) {
        self.attack_timing.frames_since_last_attack = 0;
    }

    /// Called when expansion is completed
    pub fn on_expansion_complete(&mut self) {
        self.expansion.on_expansion_complete();
    }
}

impl Default for StrategicDecisionMaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attack_timing() {
        let mut timing = AttackTiming::new();
        timing.min_military_strength = 0.8;
        timing.min_army_size = 15;
        timing.frames_since_last_attack = 500;
        timing.attack_confidence = 0.7;
        timing.enemy_strength = 0.6;

        assert!(timing.is_ready_to_attack());
    }

    #[test]
    fn test_attack_timing_not_ready() {
        let timing = AttackTiming::new();
        // Default values should not be ready
        assert!(!timing.is_ready_to_attack());
    }

    #[test]
    fn test_expansion_strategy() {
        let expansion = ExpansionStrategy::new();

        // Should be able to expand with good conditions
        assert!(expansion.should_expand(3000, 500.0, 0.9));

        // Should not expand with enemy too close
        assert!(!expansion.should_expand(3000, 300.0, 0.9));

        // Should not expand with low resources
        assert!(!expansion.should_expand(1000, 500.0, 0.9));
    }

    #[test]
    fn test_resource_management() {
        let mut resources = ResourceManagementStrategy::new();
        resources.update(5000, 4000, 1000);

        assert!(resources.is_wealthy);
        assert!(!resources.is_poor);
        assert!(!resources.economic_pressure);
    }

    #[test]
    fn test_strategic_decision_maker() {
        let mut sdm = StrategicDecisionMaker::new();
        sdm.update(100);

        let decision = sdm.make_decision(0.8, 0.5, 1.0, 0.3, 3000);
        assert!(matches!(
            decision,
            StrategicDecision::LaunchAttack | StrategicDecision::BuildUpForces
        ));
    }

    #[test]
    fn test_phase_transitions() {
        let mut sdm = StrategicDecisionMaker::new();

        sdm.update(1000); // Early game
        assert_eq!(sdm.current_phase, GamePhase::Early);

        sdm.update(10000); // Mid game
        assert_eq!(sdm.current_phase, GamePhase::Mid);

        sdm.update(30000); // Late game
        assert_eq!(sdm.current_phase, GamePhase::Late);
    }

    #[test]
    fn test_attack_confidence_calculation() {
        let mut timing = AttackTiming::new();

        // Strong position should give high confidence
        timing.calculate_attack_confidence(1.0, 0.5, 1.0, 0.2);
        assert!(timing.attack_confidence > 0.8);

        // Weak position should give low confidence
        timing.calculate_attack_confidence(0.5, 1.0, 0.5, 0.8);
        assert!(timing.attack_confidence < 0.3);
    }
}
