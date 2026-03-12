//! Target Prioritization System
//!
//! Implements sophisticated target selection and prioritization for AI units.
//! Considers factors such as:
//! - Threat level
//! - Strategic value
//! - Distance
//! - Weapon effectiveness
//! - Unit health
//! - Target type

use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::HashMap;

/// Target priority score
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TargetScore {
    pub total_score: f32,
    pub threat_score: f32,
    pub value_score: f32,
    pub distance_score: f32,
    pub effectiveness_score: f32,
}

impl TargetScore {
    pub fn new() -> Self {
        Self {
            total_score: 0.0,
            threat_score: 0.0,
            value_score: 0.0,
            distance_score: 0.0,
            effectiveness_score: 0.0,
        }
    }

    pub fn calculate_total(&mut self, weights: &PrioritizationWeights) {
        self.total_score = self.threat_score * weights.threat_weight
            + self.value_score * weights.value_weight
            + self.distance_score * weights.distance_weight
            + self.effectiveness_score * weights.effectiveness_weight;
    }
}

/// Prioritization weight configuration
#[derive(Debug, Clone)]
pub struct PrioritizationWeights {
    pub threat_weight: f32,
    pub value_weight: f32,
    pub distance_weight: f32,
    pub effectiveness_weight: f32,
}

impl Default for PrioritizationWeights {
    fn default() -> Self {
        Self {
            threat_weight: 0.4,
            value_weight: 0.3,
            distance_weight: 0.2,
            effectiveness_weight: 0.1,
        }
    }
}

/// Target information for prioritization
#[derive(Debug, Clone)]
pub struct PrioritizationTarget {
    pub object_id: ObjectID,
    pub position: Coord3D,
    pub unit_type: String,
    pub health_percentage: f32,
    pub is_attacking: bool,
    pub distance_to_attacker: f32,
}

/// Target Prioritization System
#[derive(Debug)]
pub struct TargetPrioritization {
    weights: PrioritizationWeights,
    target_scores: HashMap<ObjectID, TargetScore>,
    priority_modifiers: HashMap<String, f32>, // Unit type -> priority modifier
}

impl TargetPrioritization {
    pub fn new() -> Self {
        let mut priority_modifiers = HashMap::new();

        // Set default priority modifiers for unit types
        priority_modifiers.insert("CommandCenter".to_string(), 2.0);
        priority_modifiers.insert("SupplyCenter".to_string(), 1.5);
        priority_modifiers.insert("Barracks".to_string(), 1.3);
        priority_modifiers.insert("WarFactory".to_string(), 1.5);
        priority_modifiers.insert("Airfield".to_string(), 1.5);
        priority_modifiers.insert("Superweapon".to_string(), 3.0);
        priority_modifiers.insert("PowerPlant".to_string(), 1.2);
        priority_modifiers.insert("Defense".to_string(), 1.0);
        priority_modifiers.insert("Infantry".to_string(), 0.5);
        priority_modifiers.insert("Vehicle".to_string(), 1.0);
        priority_modifiers.insert("Aircraft".to_string(), 1.2);

        Self {
            weights: PrioritizationWeights::default(),
            target_scores: HashMap::new(),
            priority_modifiers,
        }
    }

    pub fn set_weights(&mut self, weights: PrioritizationWeights) {
        self.weights = weights;
    }

    pub fn add_priority_modifier(&mut self, unit_type: String, modifier: f32) {
        self.priority_modifiers.insert(unit_type, modifier);
    }

    pub fn evaluate_target(&mut self, target: &PrioritizationTarget) -> TargetScore {
        let mut score = TargetScore::new();

        // Calculate threat score (0.0 to 1.0)
        score.threat_score = if target.is_attacking {
            0.9 + (1.0 - target.health_percentage) * 0.1 // Prioritize damaged attacking units slightly lower
        } else {
            0.3 * (1.0 - target.health_percentage) // Low threat if not attacking
        };

        // Calculate strategic value score (0.0 to 1.0)
        let base_value = self
            .priority_modifiers
            .get(&target.unit_type)
            .copied()
            .unwrap_or(1.0);
        score.value_score = (base_value / 3.0).min(1.0); // Normalize to 0-1

        // Calculate distance score (closer = higher score, 0.0 to 1.0)
        let max_range = 500.0; // Maximum consideration range
        score.distance_score = if target.distance_to_attacker < max_range {
            1.0 - (target.distance_to_attacker / max_range)
        } else {
            0.0
        };

        // Calculate weapon effectiveness score (0.0 to 1.0)
        // This would depend on attacker's weapon vs target's armor
        // For now, use a simple heuristic
        score.effectiveness_score = if target.health_percentage < 0.3 {
            1.2 // Bonus for finishing off weak targets
        } else if target.health_percentage > 0.8 {
            0.8 // Penalty for attacking fresh targets
        } else {
            1.0
        };

        // Calculate total weighted score
        score.calculate_total(&self.weights);

        // Cache score
        self.target_scores.insert(target.object_id, score);

        score
    }

    pub fn select_best_target(&mut self, targets: &[PrioritizationTarget]) -> Option<ObjectID> {
        if targets.is_empty() {
            return None;
        }

        let mut best_target = None;
        let mut best_score = 0.0;

        for target in targets {
            let score = self.evaluate_target(target);

            if score.total_score > best_score {
                best_score = score.total_score;
                best_target = Some(target.object_id);
            }
        }

        best_target
    }

    pub fn get_target_score(&self, target_id: ObjectID) -> Option<&TargetScore> {
        self.target_scores.get(&target_id)
    }

    pub fn clear_scores(&mut self) {
        self.target_scores.clear();
    }

    pub fn get_top_n_targets(
        &mut self,
        targets: &[PrioritizationTarget],
        n: usize,
    ) -> Vec<ObjectID> {
        let mut scored_targets: Vec<_> = targets
            .iter()
            .map(|t| {
                let score = self.evaluate_target(t);
                (t.object_id, score.total_score)
            })
            .collect();

        scored_targets.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored_targets
            .into_iter()
            .take(n)
            .map(|(id, _)| id)
            .collect()
    }
}

impl Default for TargetPrioritization {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_score_calculation() {
        let mut score = TargetScore::new();
        score.threat_score = 0.8;
        score.value_score = 0.6;
        score.distance_score = 0.7;
        score.effectiveness_score = 0.9;

        let weights = PrioritizationWeights::default();
        score.calculate_total(&weights);

        assert!(score.total_score > 0.0);
        assert!(score.total_score <= 1.0);
    }

    #[test]
    fn test_target_prioritization() {
        let mut system = TargetPrioritization::new();

        let target1 = PrioritizationTarget {
            object_id: 1,
            position: [100.0, 100.0, 0.0].into(),
            unit_type: "Infantry".to_string(),
            health_percentage: 1.0,
            is_attacking: false,
            distance_to_attacker: 50.0,
        };

        let target2 = PrioritizationTarget {
            object_id: 2,
            position: [150.0, 150.0, 0.0].into(),
            unit_type: "CommandCenter".to_string(),
            health_percentage: 0.8,
            is_attacking: false,
            distance_to_attacker: 100.0,
        };

        let targets = vec![target1, target2];
        let best = system.select_best_target(&targets);

        // Command center should be prioritized
        assert_eq!(best, Some(2));
    }
}
