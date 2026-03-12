//! Combat Integration
//!
//! Handles formation behavior during combat, including breaking,
//! maintaining, and reforming formations.

use super::formation_types::{FormationSettings, FormationState};
use super::{FormationError, FormationResult};
use crate::common::{Coord3D, ObjectID, Real, LOGICFRAMES_PER_SECOND};
use std::collections::{HashMap, HashSet};

/// Combat state for formation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatState {
    /// No combat
    Idle,
    /// Engaging enemies
    Engaging,
    /// Under attack
    UnderAttack,
    /// Heavy combat
    HeavyCombat,
    /// Retreating
    Retreating,
}

/// Combat behavior configuration
#[derive(Debug, Clone)]
pub struct CombatBehavior {
    /// Break formation on engagement
    pub break_on_engagement: bool,

    /// Maintain formation while attacking
    pub maintain_while_attacking: bool,

    /// Reform after combat delay (seconds)
    pub reform_after_combat_delay: Real,

    /// Maximum combat duration before forced reform (seconds)
    pub max_combat_duration: Real,

    /// Compress formation in combat
    pub compress_in_combat: bool,

    /// Compression factor
    pub combat_compression: Real,

    /// Scatter on heavy damage
    pub scatter_on_heavy_damage: bool,

    /// Heavy damage threshold (percentage)
    pub heavy_damage_threshold: Real,
}

impl Default for CombatBehavior {
    fn default() -> Self {
        Self {
            break_on_engagement: false,
            maintain_while_attacking: true,
            reform_after_combat_delay: 3.0,
            max_combat_duration: 30.0,
            compress_in_combat: true,
            combat_compression: 0.7,
            scatter_on_heavy_damage: true,
            heavy_damage_threshold: 0.5,
        }
    }
}

/// Formation tactics in combat
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationTactics {
    /// Standard combat tactics
    Standard,
    /// Aggressive advance
    Aggressive,
    /// Defensive stance
    Defensive,
    /// Flanking maneuver
    Flanking,
    /// Kiting (hit and run)
    Kiting,
    /// Scatter and engage
    Scatter,
}

/// Unit combat status
#[derive(Debug, Clone)]
struct UnitCombatStatus {
    unit_id: ObjectID,
    in_combat: bool,
    health_percentage: Real,
    last_attacked_frame: u32,
    current_target: Option<ObjectID>,
}

/// Formation combat integration
pub struct FormationCombat {
    /// Combat behavior settings
    behavior: CombatBehavior,

    /// Current combat state
    combat_state: CombatState,

    /// Unit combat status
    unit_status: HashMap<ObjectID, UnitCombatStatus>,

    /// Enemies engaging formation
    engaging_enemies: HashSet<ObjectID>,

    /// Time entered combat (frame)
    combat_start_frame: Option<u32>,

    /// Time exited combat (frame)
    combat_end_frame: Option<u32>,

    /// Current frame
    current_frame: u32,

    /// Formation state before combat
    pre_combat_state: Option<FormationState>,

    /// Current tactics
    current_tactics: FormationTactics,
}

impl FormationCombat {
    /// Create new formation combat handler
    pub fn new(behavior: CombatBehavior) -> Self {
        Self {
            behavior,
            combat_state: CombatState::Idle,
            unit_status: HashMap::new(),
            engaging_enemies: HashSet::new(),
            combat_start_frame: None,
            combat_end_frame: None,
            current_frame: 0,
            pre_combat_state: None,
            current_tactics: FormationTactics::Standard,
        }
    }

    /// Add unit to combat tracking
    pub fn add_unit(&mut self, unit_id: ObjectID, health_percentage: Real) {
        self.unit_status.insert(
            unit_id,
            UnitCombatStatus {
                unit_id,
                in_combat: false,
                health_percentage,
                last_attacked_frame: 0,
                current_target: None,
            },
        );
    }

    /// Remove unit from combat tracking
    pub fn remove_unit(&mut self, unit_id: ObjectID) {
        self.unit_status.remove(&unit_id);
    }

    /// Update unit in combat
    pub fn update_unit_combat(
        &mut self,
        unit_id: ObjectID,
        in_combat: bool,
        health_percentage: Real,
        target: Option<ObjectID>,
    ) {
        if let Some(status) = self.unit_status.get_mut(&unit_id) {
            status.in_combat = in_combat;
            status.health_percentage = health_percentage;
            status.current_target = target;

            if in_combat {
                status.last_attacked_frame = self.current_frame;
            }
        }
    }

    /// Register enemy engaging formation
    pub fn add_engaging_enemy(&mut self, enemy_id: ObjectID) {
        self.engaging_enemies.insert(enemy_id);
    }

    /// Remove engaging enemy
    pub fn remove_engaging_enemy(&mut self, enemy_id: ObjectID) {
        self.engaging_enemies.remove(&enemy_id);
    }

    /// Update combat state
    pub fn update(&mut self, frame: u32) -> FormationResult<CombatState> {
        self.current_frame = frame;

        let units_in_combat = self.unit_status.values().filter(|s| s.in_combat).count();

        let total_units = self.unit_status.len();

        let previous_state = self.combat_state;

        // Determine combat state
        self.combat_state = if units_in_combat == 0 && self.engaging_enemies.is_empty() {
            CombatState::Idle
        } else if total_units > 1 && units_in_combat > total_units / 2 {
            CombatState::HeavyCombat
        } else if !self.engaging_enemies.is_empty() {
            CombatState::UnderAttack
        } else if units_in_combat > 0 {
            CombatState::Engaging
        } else {
            CombatState::Idle
        };

        // Track combat start/end
        if previous_state == CombatState::Idle && self.combat_state != CombatState::Idle {
            self.combat_start_frame = Some(frame);
        } else if previous_state != CombatState::Idle && self.combat_state == CombatState::Idle {
            self.combat_end_frame = Some(frame);
        }

        Ok(self.combat_state)
    }

    /// Should formation break?
    pub fn should_break_formation(&self) -> bool {
        match self.combat_state {
            CombatState::Idle => false,
            CombatState::Engaging => self.behavior.break_on_engagement,
            CombatState::UnderAttack | CombatState::HeavyCombat => {
                !self.behavior.maintain_while_attacking
            }
            CombatState::Retreating => true,
        }
    }

    /// Should formation reform?
    pub fn should_reform_formation(&self) -> bool {
        if self.combat_state != CombatState::Idle {
            return false;
        }

        if let Some(end_frame) = self.combat_end_frame {
            let frames_since_combat = self.current_frame.saturating_sub(end_frame);
            let seconds_since_combat = frames_since_combat as Real / LOGICFRAMES_PER_SECOND as Real;

            return seconds_since_combat >= self.behavior.reform_after_combat_delay;
        }

        false
    }

    /// Get combat compression factor
    pub fn get_combat_compression(&self) -> Real {
        if self.behavior.compress_in_combat && self.combat_state != CombatState::Idle {
            self.behavior.combat_compression
        } else {
            1.0
        }
    }

    /// Check if should scatter due to heavy damage
    pub fn should_scatter(&self) -> bool {
        if !self.behavior.scatter_on_heavy_damage {
            return false;
        }

        // Calculate average health
        if self.unit_status.is_empty() {
            return false;
        }

        let total_health: Real = self.unit_status.values().map(|s| s.health_percentage).sum();

        let average_health = total_health / self.unit_status.len() as Real;

        average_health < self.behavior.heavy_damage_threshold
    }

    /// Get combat state
    pub fn get_combat_state(&self) -> CombatState {
        self.combat_state
    }

    /// Set tactics
    pub fn set_tactics(&mut self, tactics: FormationTactics) {
        self.current_tactics = tactics;
    }

    /// Get current tactics
    pub fn get_tactics(&self) -> FormationTactics {
        self.current_tactics
    }

    /// Get recommended formation state for current combat
    pub fn get_recommended_formation_state(&self) -> FormationState {
        match self.combat_state {
            CombatState::Idle => {
                if self.should_reform_formation() {
                    FormationState::Reforming
                } else {
                    FormationState::Formed
                }
            }
            CombatState::Engaging | CombatState::UnderAttack => {
                if self.should_break_formation() {
                    FormationState::Breaking
                } else {
                    FormationState::InCombat
                }
            }
            CombatState::HeavyCombat => {
                if self.should_scatter() {
                    FormationState::Breaking
                } else {
                    FormationState::InCombat
                }
            }
            CombatState::Retreating => FormationState::Breaking,
        }
    }

    /// Get units needing reformation
    pub fn get_units_needing_reform(&self) -> Vec<ObjectID> {
        self.unit_status
            .values()
            .filter(|s| !s.in_combat)
            .map(|s| s.unit_id)
            .collect()
    }

    /// Get units in combat
    pub fn get_units_in_combat(&self) -> Vec<ObjectID> {
        self.unit_status
            .values()
            .filter(|s| s.in_combat)
            .map(|s| s.unit_id)
            .collect()
    }

    /// Get formation health percentage
    pub fn get_formation_health(&self) -> Real {
        if self.unit_status.is_empty() {
            return 1.0;
        }

        let total_health: Real = self.unit_status.values().map(|s| s.health_percentage).sum();

        total_health / self.unit_status.len() as Real
    }

    /// Check if formation has been in combat too long
    pub fn is_combat_duration_exceeded(&self) -> bool {
        if let Some(start_frame) = self.combat_start_frame {
            if self.combat_state != CombatState::Idle {
                let combat_duration =
                    (self.current_frame - start_frame) as Real / LOGICFRAMES_PER_SECOND as Real;
                return combat_duration > self.behavior.max_combat_duration;
            }
        }
        false
    }

    /// Save pre-combat state
    pub fn save_pre_combat_state(&mut self, state: FormationState) {
        if self.pre_combat_state.is_none() {
            self.pre_combat_state = Some(state);
        }
    }

    /// Get pre-combat state
    pub fn get_pre_combat_state(&self) -> Option<FormationState> {
        self.pre_combat_state
    }

    /// Clear combat data
    pub fn clear(&mut self) {
        self.unit_status.clear();
        self.engaging_enemies.clear();
        self.combat_state = CombatState::Idle;
        self.combat_start_frame = None;
        self.combat_end_frame = None;
        self.pre_combat_state = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_state_tracking() {
        let behavior = CombatBehavior::default();
        let mut combat = FormationCombat::new(behavior);

        combat.add_unit(100, 1.0);
        combat.add_unit(101, 1.0);

        assert_eq!(combat.get_combat_state(), CombatState::Idle);

        combat.update_unit_combat(100, true, 0.9, Some(200));
        combat.update(1).unwrap();

        assert_eq!(combat.get_combat_state(), CombatState::Engaging);
    }

    #[test]
    fn test_should_break_formation() {
        let mut behavior = CombatBehavior::default();
        behavior.break_on_engagement = true;

        let mut combat = FormationCombat::new(behavior);
        combat.add_unit(100, 1.0);

        combat.update_unit_combat(100, true, 0.9, Some(200));
        combat.update(1).unwrap();

        assert!(combat.should_break_formation());
    }

    #[test]
    fn test_heavy_damage_scatter() {
        let behavior = CombatBehavior::default();
        let mut combat = FormationCombat::new(behavior);

        combat.add_unit(100, 0.3);
        combat.add_unit(101, 0.4);

        assert!(combat.should_scatter());
    }

    #[test]
    fn test_reform_after_combat() {
        let mut behavior = CombatBehavior::default();
        behavior.reform_after_combat_delay = 1.0;

        let mut combat = FormationCombat::new(behavior);
        combat.add_unit(100, 1.0);

        // Enter combat
        combat.update_unit_combat(100, true, 0.9, Some(200));
        combat.update(1).unwrap();

        // Exit combat
        combat.update_unit_combat(100, false, 0.9, None);
        combat.update(2).unwrap();

        // Wait for delay (30 frames = 1 second at 30fps)
        combat.update(33).unwrap();

        assert!(combat.should_reform_formation());
    }

    #[test]
    fn test_combat_compression() {
        let mut behavior = CombatBehavior::default();
        behavior.compress_in_combat = true;
        behavior.combat_compression = 0.8;

        let mut combat = FormationCombat::new(behavior);
        combat.add_unit(100, 1.0);

        combat.update_unit_combat(100, true, 0.9, Some(200));
        combat.update(1).unwrap();

        assert_eq!(combat.get_combat_compression(), 0.8);
    }
}
