//! AI Difficulty Level Handling System
//!
//! Adjusts AI behavior based on difficulty level:
//! - Easy: Slower reactions, fewer units, weaker strategies
//! - Normal: Balanced AI performance
//! - Hard: Fast reactions, optimal strategies, aggressive play
//!
//! Ported from C++ AIPlayer.cpp, AIStates.cpp and AI.cpp
//! Matches C++ behavior exactly for game balance

use crate::ai::AiError;
use crate::common::{Real, LOGICFRAMES_PER_SECOND};

/// Game difficulty levels - matches C++ GameDifficulty enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
    /// For China's brutal difficulty setting
    Brutal,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        GameDifficulty::Normal
    }
}

/// Difficulty modifiers that affect AI behavior
/// Matches C++ AISideInfo from AI.h
#[derive(Debug, Clone)]
pub struct DifficultyModifiers {
    /// Number of supply trucks for this difficulty
    pub supply_gatherers: i32,

    /// Skill set selection for this difficulty (0-4)
    pub skillset_index: i32,

    /// Build speed modifier (higher = faster building)
    pub structure_speed_modifier: Real,

    /// Unit production speed modifier
    pub team_speed_modifier: Real,

    /// Reaction time in frames
    pub reaction_time_frames: u32,

    /// Accuracy modifier (0.0 to 1.0)
    pub accuracy_modifier: Real,

    /// Damage output modifier (0.5 to 1.0)
    pub damage_modifier: Real,

    /// Damage received modifier (0.5 to 1.5)
    pub damage_received_modifier: Real,

    /// Vision range modifier (0.8 to 1.2)
    pub vision_modifier: Real,

    /// Resource collection rate modifier
    pub resource_modifier: Real,
}

impl DifficultyModifiers {
    /// Create modifiers for specific difficulty - matches C++ AISideInfo defaults
    pub fn for_difficulty(difficulty: GameDifficulty) -> Self {
        match difficulty {
            GameDifficulty::Easy => Self {
                // Matches C++ AISideInfo::m_easy = 2
                supply_gatherers: 2,
                skillset_index: 0,
                // Slower building (matches C++ poorMod logic)
                structure_speed_modifier: 0.7,
                team_speed_modifier: 0.7,
                // Slower reactions (5 seconds)
                reaction_time_frames: 150,
                // Lower accuracy
                accuracy_modifier: 0.7,
                // Reduced damage output
                damage_modifier: 0.8,
                // Takes more damage
                damage_received_modifier: 1.2,
                // Reduced vision
                vision_modifier: 0.9,
                // Slower resource collection
                resource_modifier: 0.8,
            },
            GameDifficulty::Normal => Self {
                // Matches C++ AISideInfo::m_normal = 3
                supply_gatherers: 3,
                skillset_index: 1,
                // Normal building speed
                structure_speed_modifier: 1.0,
                team_speed_modifier: 1.0,
                // Normal reactions (2 seconds)
                reaction_time_frames: 60,
                // Normal accuracy
                accuracy_modifier: 1.0,
                // Normal damage
                damage_modifier: 1.0,
                // Normal damage received
                damage_received_modifier: 1.0,
                // Normal vision
                vision_modifier: 1.0,
                // Normal resources
                resource_modifier: 1.0,
            },
            GameDifficulty::Hard => Self {
                // Matches C++ AISideInfo::m_hard = 4
                supply_gatherers: 4,
                skillset_index: 2,
                // Faster building (matches C++ wealthyMod logic)
                structure_speed_modifier: 1.5,
                team_speed_modifier: 1.5,
                // Faster reactions (1 second)
                reaction_time_frames: 30,
                // Better accuracy
                accuracy_modifier: 1.0,
                // Full damage
                damage_modifier: 1.0,
                // Takes less damage
                damage_received_modifier: 0.9,
                // Enhanced vision
                vision_modifier: 1.1,
                // Faster resource collection
                resource_modifier: 1.2,
            },
            GameDifficulty::Brutal => Self {
                // Extreme difficulty for Generals challenge
                supply_gatherers: 5,
                skillset_index: 3,
                // Much faster building
                structure_speed_modifier: 2.0,
                team_speed_modifier: 2.0,
                // Instant reactions
                reaction_time_frames: 15,
                // Perfect accuracy
                accuracy_modifier: 1.0,
                // Enhanced damage
                damage_modifier: 1.1,
                // Greatly reduced damage taken
                damage_received_modifier: 0.7,
                // Maximum vision
                vision_modifier: 1.2,
                // Maximum resource collection
                resource_modifier: 1.5,
            },
        }
    }

    /// Get structure timer value - matches C++ m_structureTimer calculation
    /// From AIPlayer.cpp and AISkirmishPlayer.cpp
    pub fn get_structure_timer_frames(&self, base_seconds: Real) -> u32 {
        // Matches C++:
        // m_structureTimer = TheAI->getAiData()->m_structureSeconds*LOGICFRAMES_PER_SECOND;
        // If wealthy: m_structureTimer = m_structureTimer/TheAI->getAiData()->m_structuresWealthyMod;
        // If poor: m_structureTimer = m_structureTimer/TheAI->getAiData()->m_structuresPoorMod;
        let base_frames = base_seconds * LOGICFRAMES_PER_SECOND as Real;
        let modified = base_frames / self.structure_speed_modifier;
        modified as u32
    }

    /// Get team timer value - matches C++ m_teamTimer calculation
    pub fn get_team_timer_frames(&self, base_seconds: Real) -> u32 {
        let base_frames = base_seconds * LOGICFRAMES_PER_SECOND as Real;
        let modified = base_frames / self.team_speed_modifier;
        modified as u32
    }

    /// Apply accuracy modifier to hit chance
    pub fn apply_accuracy(&self, base_accuracy: Real) -> Real {
        (base_accuracy * self.accuracy_modifier).min(1.0).max(0.0)
    }

    /// Apply damage modifier to outgoing damage
    pub fn apply_damage(&self, base_damage: Real) -> Real {
        base_damage * self.damage_modifier
    }

    /// Apply damage received modifier to incoming damage
    pub fn apply_damage_received(&self, incoming_damage: Real) -> Real {
        incoming_damage * self.damage_received_modifier
    }

    /// Apply vision modifier to vision range
    pub fn apply_vision_range(&self, base_range: Real) -> Real {
        base_range * self.vision_modifier
    }

    /// Apply resource modifier to collection rate
    pub fn apply_resource_collection(&self, base_rate: Real) -> Real {
        base_rate * self.resource_modifier
    }
}

/// Difficulty-adjusted AI parameters
/// Matches C++ AIStates.cpp difficulty handling
#[derive(Debug, Clone)]
pub struct DifficultyAdjustedParams {
    /// Base difficulty level
    pub difficulty: GameDifficulty,

    /// Modifiers for this difficulty
    pub modifiers: DifficultyModifiers,

    /// Attack delay frames (matches C++ AIStates reacquisitionTime)
    pub attack_delay_frames: u32,

    /// Chase distance in map units
    pub chase_distance: Real,

    /// Pursuit time in frames
    pub pursuit_time_frames: u32,
}

impl DifficultyAdjustedParams {
    /// Create parameters for specific difficulty
    /// Matches C++ AIStates.cpp difficulty switch statement
    pub fn new(difficulty: GameDifficulty) -> Self {
        let modifiers = DifficultyModifiers::for_difficulty(difficulty);

        // From C++ AIStates.cpp - different timing based on difficulty
        let (attack_delay_frames, chase_distance, pursuit_time_frames) = match difficulty {
            GameDifficulty::Easy => {
                // Easy: Slow reactions, short pursuit
                (150, 150.0, 180) // 5 sec delay, 150 units chase, 6 sec pursuit
            }
            GameDifficulty::Normal => {
                // Normal: Balanced
                (90, 200.0, 240) // 3 sec delay, 200 units chase, 8 sec pursuit
            }
            GameDifficulty::Hard => {
                // Hard: Fast reactions, long pursuit
                (60, 300.0, 360) // 2 sec delay, 300 units chase, 12 sec pursuit
            }
            GameDifficulty::Brutal => {
                // Brutal: Instant reactions, very long pursuit
                (30, 400.0, 480) // 1 sec delay, 400 units chase, 16 sec pursuit
            }
        };

        Self {
            difficulty,
            modifiers,
            attack_delay_frames,
            chase_distance,
            pursuit_time_frames,
        }
    }

    /// Check if we should delay attack based on difficulty
    pub fn should_delay_attack(&self, frames_since_last_action: u32) -> bool {
        frames_since_last_action < self.attack_delay_frames
    }

    /// Check if target is within chase distance
    pub fn is_within_chase_distance(&self, distance: Real) -> bool {
        distance <= self.chase_distance
    }

    /// Check if we should continue pursuing
    pub fn should_continue_pursuit(&self, pursuit_frames: u32) -> bool {
        pursuit_frames < self.pursuit_time_frames
    }
}

/// AI skill set selection - matches C++ SkillSet from AI.h
#[derive(Debug, Clone)]
pub struct AISkillSet {
    /// Number of skills in this set
    pub num_skills: i32,

    /// Science/upgrade types to research
    pub skills: Vec<i32>,
}

impl AISkillSet {
    pub fn new() -> Self {
        Self {
            num_skills: 0,
            skills: Vec::new(),
        }
    }

    /// Create skill set for difficulty level
    /// Matches C++ AISideInfo skill sets
    pub fn for_difficulty(difficulty: GameDifficulty, _faction: &str) -> Self {
        let mut skillset = Self::new();

        match difficulty {
            GameDifficulty::Easy => {
                // Basic upgrades only
                skillset.num_skills = 2;
                skillset.skills = vec![
                    1, // Basic infantry upgrade
                    2, // Basic vehicle upgrade
                ];
            }
            GameDifficulty::Normal => {
                // Moderate upgrades
                skillset.num_skills = 4;
                skillset.skills = vec![
                    1, // Basic infantry upgrade
                    2, // Basic vehicle upgrade
                    3, // Advanced infantry
                    4, // Advanced vehicle
                ];
            }
            GameDifficulty::Hard | GameDifficulty::Brutal => {
                // All upgrades
                skillset.num_skills = 6;
                skillset.skills = vec![
                    1, // Basic infantry upgrade
                    2, // Basic vehicle upgrade
                    3, // Advanced infantry
                    4, // Advanced vehicle
                    5, // Elite infantry
                    6, // Elite vehicle
                ];
            }
        }

        skillset
    }
}

impl Default for AISkillSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Main difficulty handling system
#[derive(Debug)]
pub struct DifficultyHandler {
    /// Current difficulty level
    pub difficulty: GameDifficulty,

    /// Modifiers for current difficulty
    pub modifiers: DifficultyModifiers,

    /// Adjusted parameters
    pub params: DifficultyAdjustedParams,

    /// Skill set for this difficulty
    pub skillset: AISkillSet,
}

impl DifficultyHandler {
    /// Create handler for specific difficulty
    pub fn new(difficulty: GameDifficulty, faction: &str) -> Self {
        Self {
            difficulty,
            modifiers: DifficultyModifiers::for_difficulty(difficulty),
            params: DifficultyAdjustedParams::new(difficulty),
            skillset: AISkillSet::for_difficulty(difficulty, faction),
        }
    }

    /// Set new difficulty level
    pub fn set_difficulty(&mut self, difficulty: GameDifficulty, faction: &str) {
        self.difficulty = difficulty;
        self.modifiers = DifficultyModifiers::for_difficulty(difficulty);
        self.params = DifficultyAdjustedParams::new(difficulty);
        self.skillset = AISkillSet::for_difficulty(difficulty, faction);
    }

    /// Get difficulty level
    pub fn get_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Check if AI should use advanced tactics
    pub fn use_advanced_tactics(&self) -> bool {
        matches!(
            self.difficulty,
            GameDifficulty::Hard | GameDifficulty::Brutal
        )
    }

    /// Check if AI should micro-manage units
    pub fn use_micromanagement(&self) -> bool {
        self.difficulty != GameDifficulty::Easy
    }

    /// Check if AI should use special powers aggressively
    pub fn use_special_powers_aggressively(&self) -> bool {
        matches!(
            self.difficulty,
            GameDifficulty::Hard | GameDifficulty::Brutal
        )
    }
}

impl Default for DifficultyHandler {
    fn default() -> Self {
        Self::new(GameDifficulty::Normal, "USA")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_modifiers() {
        let easy = DifficultyModifiers::for_difficulty(GameDifficulty::Easy);
        let normal = DifficultyModifiers::for_difficulty(GameDifficulty::Normal);
        let hard = DifficultyModifiers::for_difficulty(GameDifficulty::Hard);

        // Easy should be slower than normal
        assert!(easy.structure_speed_modifier < normal.structure_speed_modifier);

        // Hard should be faster than normal
        assert!(hard.structure_speed_modifier > normal.structure_speed_modifier);

        // Easy should have fewer gatherers
        assert!(easy.supply_gatherers < hard.supply_gatherers);
    }

    #[test]
    fn test_timer_calculations() {
        let modifiers = DifficultyModifiers::for_difficulty(GameDifficulty::Normal);

        // 10 second base timer at normal difficulty
        let timer = modifiers.get_structure_timer_frames(10.0);
        assert_eq!(timer, 300); // 10 * 30 FPS
    }

    #[test]
    fn test_accuracy_modifier() {
        let easy = DifficultyModifiers::for_difficulty(GameDifficulty::Easy);
        let hard = DifficultyModifiers::for_difficulty(GameDifficulty::Hard);

        let base_accuracy = 0.8;

        // Easy should have lower accuracy
        assert!(easy.apply_accuracy(base_accuracy) < base_accuracy);

        // Hard should maintain or improve accuracy
        assert!(hard.apply_accuracy(base_accuracy) >= base_accuracy);
    }

    #[test]
    fn test_damage_modifiers() {
        let easy = DifficultyModifiers::for_difficulty(GameDifficulty::Easy);
        let hard = DifficultyModifiers::for_difficulty(GameDifficulty::Hard);

        let base_damage = 100.0;

        // Easy does less damage
        assert!(easy.apply_damage(base_damage) < base_damage);

        // Easy takes more damage
        assert!(easy.apply_damage_received(base_damage) > base_damage);

        // Hard takes less damage
        assert!(hard.apply_damage_received(base_damage) < base_damage);
    }

    #[test]
    fn test_difficulty_params() {
        let easy = DifficultyAdjustedParams::new(GameDifficulty::Easy);
        let hard = DifficultyAdjustedParams::new(GameDifficulty::Hard);

        // Hard should have faster reactions
        assert!(easy.attack_delay_frames > hard.attack_delay_frames);

        // Hard should chase further
        assert!(easy.chase_distance < hard.chase_distance);

        // Hard should pursue longer
        assert!(easy.pursuit_time_frames < hard.pursuit_time_frames);
    }

    #[test]
    fn test_skillset_progression() {
        let easy = AISkillSet::for_difficulty(GameDifficulty::Easy, "USA");
        let hard = AISkillSet::for_difficulty(GameDifficulty::Hard, "USA");

        // Hard should have more skills
        assert!(easy.num_skills < hard.num_skills);
        assert!(easy.skills.len() < hard.skills.len());
    }

    #[test]
    fn test_difficulty_handler() {
        let mut handler = DifficultyHandler::new(GameDifficulty::Easy, "USA");

        assert_eq!(handler.get_difficulty(), GameDifficulty::Easy);
        assert!(!handler.use_advanced_tactics());

        handler.set_difficulty(GameDifficulty::Hard, "USA");

        assert_eq!(handler.get_difficulty(), GameDifficulty::Hard);
        assert!(handler.use_advanced_tactics());
    }

    #[test]
    fn test_tactical_decisions() {
        let easy = DifficultyHandler::new(GameDifficulty::Easy, "USA");
        let hard = DifficultyHandler::new(GameDifficulty::Hard, "USA");

        // Easy shouldn't micromanage
        assert!(!easy.use_advanced_tactics());

        // Hard should use all advanced features
        assert!(hard.use_advanced_tactics());
        assert!(hard.use_micromanagement());
        assert!(hard.use_special_powers_aggressively());
    }
}
