//! AI Update Modules - Specialized AI behaviors for different unit types
//!
//! This module contains specialized AI update modules that control the behavior
//! of different types of units and structures. Each module implements specific
//! logic for its unit type, including:
//!
//! - DozerAI: Construction vehicle behavior (building, repairing)
//! - SupplyTruckAI: Resource collection and delivery
//! - WorkerAI: Chinese worker unit behavior
//! - TransportAI: Troop transport and evacuation
//! - ChinookAI: Supply chinook behavior
//! - JetAI: Fighter jet and aircraft behavior
//! - TurretAI: Base defense turret targeting
//! - WanderAI: Idle unit exploration
//! - DeployStyleAI: Unit deployment mechanics
//!
//! Author: Converted from C++ original

pub mod ai_update_base;
pub mod chinook_ai;
pub mod deploy_style_ai;
pub mod dozer_ai;
pub mod jet_ai;
pub mod supply_truck_ai;
pub mod transport_ai;
pub mod turret_ai;
pub mod wander_ai;
pub mod worker_ai;

// Strategic AI modules
pub mod build_order;
pub mod difficulty_handling;
pub mod strategic_decision;
pub mod target_prioritization;
pub mod tech_progression;
pub mod threat_assessment;

// Re-export commonly used types
pub use ai_update_base::{AIUpdateModule, AIUpdateModuleTrait, AIUpdateResult};
pub use chinook_ai::ChinookAIUpdate;
pub use deploy_style_ai::DeployStyleAIUpdate;
pub use dozer_ai::DozerAIUpdate;
pub use jet_ai::JetAIUpdate;
pub use supply_truck_ai::{SupplyTruckAIUpdate, SupplyTruckAIUpdateData};
pub use transport_ai::TransportAIUpdate;
pub use turret_ai::TurretAIUpdate;
pub use wander_ai::WanderAIUpdate;
pub use worker_ai::WorkerAIUpdate;

pub use build_order::BuildOrderOptimizer;
pub use difficulty_handling::{
    AISkillSet, DifficultyAdjustedParams, DifficultyHandler, DifficultyModifiers, GameDifficulty,
};
pub use strategic_decision::{
    AttackTiming, ExpansionStrategy, GamePhase, ResourceManagementStrategy, StrategicDecision,
    StrategicDecisionMaker, StrategicStance,
};
pub use target_prioritization::TargetPrioritization;
pub use tech_progression::TechProgressionManager;
pub use threat_assessment::ThreatAssessmentSystem;

use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real, SECONDS_PER_LOGICFRAME_REAL};

/// AI Update module types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIModuleType {
    Base,
    Dozer,
    SupplyTruck,
    Worker,
    Transport,
    Chinook,
    Jet,
    Turret,
    Wander,
    DeployStyle,
}

/// AI Update module priority for execution order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AIModulePriority {
    Critical = 0, // Must execute first (combat, survival)
    High = 1,     // Important behaviors (resource gathering, construction)
    Normal = 2,   // Standard behaviors (movement, idle)
    Low = 3,      // Optional behaviors (exploration, patrol)
}

/// AI module state for tracking behavior state machines
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIModuleState {
    Idle,     // Not doing anything
    Active,   // Currently executing
    Waiting,  // Waiting for conditions
    Blocked,  // Blocked by obstacles or conditions
    Complete, // Task complete
    Failed,   // Task failed
}

/// Common AI update context shared by all modules
#[derive(Debug, Clone)]
pub struct AIUpdateContext {
    pub object_id: ObjectID,
    pub current_frame: u32,
    pub delta_time: Real,
    pub position: Coord3D,
    pub health_percentage: f32,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub current_target: Option<ObjectID>,
}

impl AIUpdateContext {
    pub fn new(object_id: ObjectID, current_frame: u32) -> Self {
        Self {
            object_id,
            current_frame,
            delta_time: SECONDS_PER_LOGICFRAME_REAL,
            position: Coord3D::new(0.0, 0.0, 0.0),
            health_percentage: 1.0,
            is_moving: false,
            is_attacking: false,
            current_target: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_module_types() {
        assert_eq!(AIModuleType::Dozer, AIModuleType::Dozer);
        assert_ne!(AIModuleType::Dozer, AIModuleType::SupplyTruck);
    }

    #[test]
    fn test_ai_module_priority() {
        assert!(AIModulePriority::Critical < AIModulePriority::High);
        assert!(AIModulePriority::High < AIModulePriority::Normal);
        assert!(AIModulePriority::Normal < AIModulePriority::Low);
    }

    #[test]
    fn test_ai_update_context() {
        let context = AIUpdateContext::new(123, 100);
        assert_eq!(context.object_id, 123);
        assert_eq!(context.current_frame, 100);
        assert_eq!(context.health_percentage, 1.0);
    }
}

// Comprehensive test module
#[cfg(test)]
mod tests_comprehensive;
#[cfg(test)]
pub use tests_comprehensive::*;
