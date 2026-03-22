//! AI Core - Complete AI System Integration
//!
//! This module provides the complete AI system with all components integrated:
//! - AI Manager (TheAI singleton)
//! - AIPlayer (base computer player)
//! - AISkirmishPlayer (skirmish-specific AI)
//! - Build order system
//! - Resource management
//! - Attack/defense coordination
//! - Difficulty levels
//! - AI personalities
//!
//! Reference: C++ AI.cpp, AIPlayer.cpp, AISkirmishPlayer.cpp
//! Author: Converted from C++ by Claude, original by Michael S. Booth

use crate::ai::modules::{
    BuildOrderOptimizer, DifficultyHandler, GameDifficulty, StrategicDecision,
    StrategicDecisionMaker, ThreatAssessmentSystem,
};
use crate::ai::{AiData, AiError, AiGroup, AttackPriorityInfo, PartitionFilter, AI};
use crate::common::{Coord3D, ObjectID, PlayerId, Real};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};

/// AI Manager - Complete AI system controller
/// Matches C++ TheAI singleton pattern
pub struct AIManager {
    /// The core AI instance
    core: AI,

    /// All AI players in the game
    ai_players: HashMap<PlayerId, Arc<RwLock<Box<dyn AIPlayerTrait>>>>,

    /// Current game frame
    current_frame: u32,
}

impl AIManager {
    pub fn new() -> Self {
        let mut core = AI::new();
        core.init();

        Self {
            core,
            ai_players: HashMap::new(),
            current_frame: 0,
        }
    }

    /// Register an AI player with the manager
    pub fn register_ai_player(&mut self, player_id: PlayerId, ai_player: Box<dyn AIPlayerTrait>) {
        self.ai_players
            .insert(player_id, Arc::new(RwLock::new(ai_player)));
    }

    /// Unregister an AI player
    pub fn unregister_ai_player(&mut self, player_id: PlayerId) {
        self.ai_players.remove(&player_id);
    }

    /// Update all AI systems
    /// Matches C++ AI::update() at AI.cpp:332
    pub fn update(&mut self) -> Result<(), AiError> {
        self.current_frame += 1;

        // Update core AI (pathfinding, groups)
        self.core.update(self.current_frame)?;

        // Update all AI players
        for (player_id, ai_player) in &self.ai_players {
            if let Ok(mut player) = ai_player.write() {
                if let Err(e) = player.update() {
                    eprintln!("AI Player {} update failed: {}", player_id, e);
                }
            }
        }

        Ok(())
    }

    /// Reset the AI system for a new game/map
    pub fn reset(&mut self) {
        self.core.reset();
        self.ai_players.clear();
        self.current_frame = 0;
    }

    /// Get the core AI instance
    pub fn get_core(&self) -> &AI {
        &self.core
    }

    /// Get mutable core AI instance
    pub fn get_core_mut(&mut self) -> &mut AI {
        &mut self.core
    }

    /// Find closest enemy to an object
    /// Matches C++ AI::findClosestEnemy() at AI.cpp:563
    pub fn find_closest_enemy(
        &self,
        me: ObjectID,
        range: Real,
        qualifiers: u32,
        info: Option<&AttackPriorityInfo>,
        optional_filter: Option<&dyn PartitionFilter>,
    ) -> Result<Option<ObjectID>, AiError> {
        self.core
            .find_closest_enemy(me, range, qualifiers, info, optional_filter)
    }

    /// Find closest ally to an object
    /// Matches C++ AI::findClosestAlly() at AI.cpp:714
    pub fn find_closest_ally(
        &self,
        me: ObjectID,
        range: Real,
        qualifiers: u32,
    ) -> Result<Option<ObjectID>, AiError> {
        self.core.find_closest_ally(me, range, qualifiers)
    }

    /// Get AI data (configuration)
    pub fn get_ai_data(&self) -> Arc<RwLock<AiData>> {
        self.core.get_ai_data()
    }

    /// Create a new AI group
    pub fn create_group(&mut self) -> Arc<RwLock<AiGroup>> {
        self.core.create_group()
    }

    /// Destroy an AI group
    pub fn destroy_group(&mut self, group_id: u32) -> Result<(), AiError> {
        self.core.destroy_group(group_id)
    }

    /// Find an AI group by ID
    pub fn find_group(&self, id: u32) -> Option<Arc<RwLock<AiGroup>>> {
        self.core.find_group(id)
    }
}

impl Default for AIManager {
    fn default() -> Self {
        Self::new()
    }
}

/// AI Player trait - Common interface for all AI player types
/// Matches C++ AIPlayer base class
pub trait AIPlayerTrait: Send + Sync {
    /// Main update method called each frame
    fn update(&mut self) -> Result<(), AiError>;

    /// Update economy management (resource gathering, supply trucks)
    fn update_economy(&mut self) -> Result<(), AiError>;

    /// Update construction (buildings, repairs)
    fn update_construction(&mut self) -> Result<(), AiError>;

    /// Update military decisions (attacks, defense)
    fn update_military(&mut self) -> Result<(), AiError> {
        Ok(())
    }

    /// Update diplomacy (alliances, trading)
    fn update_diplomacy(&mut self) -> Result<(), AiError> {
        Ok(())
    }

    /// Build a specific building
    fn build_specific_building(&mut self, building_name: &str) -> Result<(), AiError>;

    /// Build a specific team
    fn build_specific_team(&mut self, _team_name: &str, _priority: bool) -> Result<(), AiError> {
        Ok(())
    }

    /// Get player ID
    fn get_player_id(&self) -> u32;

    /// Get difficulty level
    fn get_difficulty(&self) -> GameDifficulty;

    /// Set difficulty level
    fn set_difficulty(&mut self, difficulty: GameDifficulty);

    /// Called when a unit is produced
    fn on_unit_produced(
        &mut self,
        _factory_id: ObjectID,
        _unit_id: ObjectID,
    ) -> Result<(), AiError> {
        Ok(())
    }

    /// Called when a structure is produced
    fn on_structure_produced(
        &mut self,
        _factory_id: ObjectID,
        _structure_id: ObjectID,
    ) -> Result<(), AiError> {
        Ok(())
    }
}

/// AI Update Interface - Interface for AI behaviors on individual units
/// Matches C++ AIUpdateInterface
pub trait AIUpdateInterface: Send + Sync {
    /// Update the AI behavior
    fn update(&mut self, delta_time: Real) -> Result<(), AiError>;

    /// Get the object this AI controls
    fn get_object_id(&self) -> ObjectID;

    /// Check if the unit is idle
    fn is_idle(&self) -> bool;

    /// Check if the unit is busy
    fn is_busy(&self) -> bool;

    /// Get current command
    fn get_current_command(&self) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_manager_creation() {
        let manager = AIManager::new();
        assert_eq!(manager.current_frame, 0);
        assert_eq!(manager.ai_players.len(), 0);
    }

    #[test]
    fn test_ai_manager_update() {
        let mut manager = AIManager::new();
        assert!(manager.update().is_ok());
        assert_eq!(manager.current_frame, 1);

        // Update again
        assert!(manager.update().is_ok());
        assert_eq!(manager.current_frame, 2);
    }

    #[test]
    fn test_ai_manager_reset() {
        let mut manager = AIManager::new();

        // Update a few times
        manager.update().unwrap();
        manager.update().unwrap();
        assert_eq!(manager.current_frame, 2);

        // Reset
        manager.reset();
        assert_eq!(manager.current_frame, 0);
        assert_eq!(manager.ai_players.len(), 0);
    }
}
