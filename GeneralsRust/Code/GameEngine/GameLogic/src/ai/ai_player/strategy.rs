//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

/// AI strategy state information
#[derive(Debug, Clone, Default)]
pub struct AiStrategyState {
    pub current_strategy: AiStrategy,
    pub strategy_confidence: f32,              // 0.0 to 1.0
    pub time_in_strategy: u32,                 // Frames in current strategy
    pub last_strategy_change: u32,             // Frame of last strategy change
    pub fallback_strategy: Option<AiStrategy>, // Backup strategy
}

/// AI strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiStrategy {
    Turtle,     // Defensive, build up base
    Rush,       // Early aggressive attack
    Economic,   // Focus on resource gathering
    Balanced,   // Balanced approach
    Expansion,  // Expand territory
    TechRush,   // Focus on technology advancement
    Harassment, // Hit and run tactics
    AllOut,     // All-out attack
}

impl Default for AiStrategy {
    fn default() -> Self {
        AiStrategy::Balanced
    }
}

/// Economic state tracking
#[derive(Debug, Clone, Default)]
pub struct AiEconomicState {
    pub current_resources: HashMap<String, i32>, // Resources by type
    pub resource_income_rate: HashMap<String, f32>, // Income per second
    pub resource_priorities: Vec<String>,        // Ordered by priority
    pub economic_pressure: f32,                  // 0.0 to 1.0, higher = more pressure
    pub supply_shortage: bool,                   // Running low on supplies
    pub power_shortage: bool,                    // Need more power
}

/// Military state tracking
#[derive(Debug, Clone, Default)]
pub struct AiMilitaryState {
    pub total_military_strength: f32, // Overall military power
    pub unit_counts_by_type: HashMap<String, i32>, // Unit counts
    pub preferred_unit_mix: Vec<UnitMixPreference>, // Desired unit composition
    pub current_military_stance: MilitaryStance, // Current military posture
    pub enemy_strength_estimate: f32, // Estimated enemy strength
    pub last_combat_frame: u32,       // Frame of last combat
}

/// Military stance options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilitaryStance {
    Defensive,  // Defend base and territory
    Aggressive, // Actively seek combat
    Balanced,   // Mix of offense and defense
    Retreating, // Pulling back
}

impl Default for MilitaryStance {
    fn default() -> Self {
        MilitaryStance::Balanced
    }
}

/// Unit mix preferences for army composition
#[derive(Debug, Clone)]
pub struct UnitMixPreference {
    pub unit_type: String,
    pub desired_percentage: f32, // 0.0 to 1.0
    pub minimum_count: i32,      // Minimum units of this type
    pub priority: i32,           // Build priority (lower = higher priority)
}

/// Construction priority for buildings
#[derive(Debug, Clone)]
pub struct ConstructionPriority {
    pub building_type: String,
    pub priority: i32,           // Lower = higher priority
    pub prerequisites_met: bool, // Can we build this now?
    pub max_count: Option<i32>,  // Maximum number to build
    pub current_count: i32,      // How many we have now
    pub desired_location: Option<Coord3D>,
    pub desired_angle: Option<Real>,
}

/// Threat assessment system
#[derive(Debug, Clone, Default)]
pub struct ThreatAssessment {
    pub immediate_threats: Vec<ThreatInfo>, // Threats requiring immediate response
    pub potential_threats: Vec<ThreatInfo>, // Future threats to watch
    pub overall_threat_level: f32,          // 0.0 to 1.0
    pub recommended_response: ThreatResponse, // Suggested action
}

/// Individual threat information
#[derive(Debug, Clone)]
pub struct ThreatInfo {
    pub threat_id: ObjectID,
    pub threat_type: ThreatType,
    pub location: Coord3D,
    pub severity: f32,                 // 0.0 to 1.0
    pub time_detected: u32,            // Frame when detected
    pub estimated_time_to_impact: u32, // Frames until threat reaches us
}

/// Types of threats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatType {
    Military,     // Enemy military units
    Economic,     // Threats to economy (raids on workers)
    Strategic,    // Long-term strategic threats
    Superweapon,  // Incoming superweapon
    Infiltration, // Spies, stealth units
}

/// Recommended threat responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatResponse {
    None,      // No action needed
    Monitor,   // Keep watching
    Defend,    // Build defenses
    Attack,    // Counter-attack
    Retreat,   // Pull back
    Emergency, // All-out response
}

impl Default for ThreatResponse {
    fn default() -> Self {
        ThreatResponse::None
    }
}
