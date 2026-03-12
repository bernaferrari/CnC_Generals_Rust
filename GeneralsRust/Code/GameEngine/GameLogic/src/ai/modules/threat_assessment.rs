//! Threat Assessment System
//!
//! Evaluates threats to AI player and units, providing:
//! - Threat detection and classification
//! - Threat level calculation
//! - Response recommendations
//! - Multi-threat prioritization

use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatType {
    Military,     // Enemy military forces
    Economic,     // Threats to economy
    Strategic,    // Strategic threats (superweapons, tech rushes)
    Infiltration, // Stealth/spy threats
    Harassment,   // Hit-and-run attacks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreatLevel {
    None,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatResponse {
    None,
    Monitor,
    Defend,
    CounterAttack,
    Retreat,
    Emergency,
}

#[derive(Debug, Clone)]
pub struct ThreatInfo {
    pub threat_id: ObjectID,
    pub threat_type: ThreatType,
    pub threat_level: ThreatLevel,
    pub position: Coord3D,
    pub severity: f32, // 0.0 to 1.0
    pub detection_frame: u32,
    pub last_update_frame: u32,
    pub estimated_strength: f32,
    pub distance_to_base: f32,
}

#[derive(Debug)]
pub struct ThreatAssessmentSystem {
    active_threats: HashMap<ObjectID, ThreatInfo>,
    threat_history: VecDeque<ThreatInfo>,
    max_history_size: usize,

    overall_threat_level: ThreatLevel,
    threat_score: f32,

    base_position: Coord3D,
    alert_radius: Real,
    critical_radius: Real,
}

impl ThreatAssessmentSystem {
    pub fn new() -> Self {
        Self {
            active_threats: HashMap::new(),
            threat_history: VecDeque::new(),
            max_history_size: 100,
            overall_threat_level: ThreatLevel::None,
            threat_score: 0.0,
            base_position: Coord3D::new(0.0, 0.0, 0.0),
            alert_radius: 500.0,
            critical_radius: 200.0,
        }
    }

    pub fn set_base_position(&mut self, position: Coord3D) {
        self.base_position = position;
    }

    pub fn add_threat(&mut self, threat: ThreatInfo) {
        let mut threat = threat;
        threat.distance_to_base = self.calculate_distance(threat.position, self.base_position);
        threat.threat_level = self.calculate_threat_level(&threat);
        self.active_threats.insert(threat.threat_id, threat);
        self.recalculate_overall_threat();
    }

    pub fn remove_threat(&mut self, threat_id: ObjectID) {
        if let Some(threat) = self.active_threats.remove(&threat_id) {
            self.threat_history.push_back(threat);
            if self.threat_history.len() > self.max_history_size {
                self.threat_history.pop_front();
            }
        }
        self.recalculate_overall_threat();
    }

    pub fn update_threat(
        &mut self,
        threat_id: ObjectID,
        severity: f32,
        position: Coord3D,
        frame: u32,
    ) {
        let distance = self.calculate_distance(position, self.base_position);

        // Pre-compute the threat level using an immutable snapshot to avoid borrow conflicts.
        let new_level = self.active_threats.get(&threat_id).map(|existing| {
            let mut snapshot = existing.clone();
            snapshot.severity = severity;
            snapshot.position = position;
            snapshot.last_update_frame = frame;
            snapshot.distance_to_base = distance;
            self.calculate_threat_level(&snapshot)
        });

        if let Some(threat) = self.active_threats.get_mut(&threat_id) {
            threat.severity = severity;
            threat.position = position;
            threat.last_update_frame = frame;
            threat.distance_to_base = distance;
            if let Some(level) = new_level {
                threat.threat_level = level;
            }
        }
        self.recalculate_overall_threat();
    }

    pub fn get_threat(&self, threat_id: ObjectID) -> Option<&ThreatInfo> {
        self.active_threats.get(&threat_id)
    }

    pub fn get_all_threats(&self) -> Vec<&ThreatInfo> {
        self.active_threats.values().collect()
    }

    pub fn get_threats_by_type(&self, threat_type: ThreatType) -> Vec<&ThreatInfo> {
        self.active_threats
            .values()
            .filter(|t| t.threat_type == threat_type)
            .collect()
    }

    pub fn get_overall_threat_level(&self) -> ThreatLevel {
        self.overall_threat_level
    }

    pub fn get_threat_score(&self) -> f32 {
        self.threat_score
    }

    pub fn get_recommended_response(&self) -> ThreatResponse {
        match self.overall_threat_level {
            ThreatLevel::None => ThreatResponse::None,
            ThreatLevel::Low => ThreatResponse::Monitor,
            ThreatLevel::Moderate => ThreatResponse::Defend,
            ThreatLevel::High => ThreatResponse::CounterAttack,
            ThreatLevel::Critical => ThreatResponse::Emergency,
        }
    }

    pub fn get_highest_priority_threat(&self) -> Option<&ThreatInfo> {
        self.active_threats.values().max_by(|a, b| {
            a.severity
                .partial_cmp(&b.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn clear_old_threats(&mut self, current_frame: u32, max_age: u32) {
        let threats_to_remove: Vec<ObjectID> = self
            .active_threats
            .iter()
            .filter(|(_, threat)| current_frame - threat.last_update_frame > max_age)
            .map(|(id, _)| *id)
            .collect();

        for threat_id in threats_to_remove {
            self.remove_threat(threat_id);
        }
    }

    fn calculate_distance(&self, pos1: Coord3D, pos2: Coord3D) -> Real {
        let dx = pos1[0] - pos2[0];
        let dy = pos1[1] - pos2[1];
        (dx * dx + dy * dy).sqrt()
    }

    fn calculate_threat_level(&self, threat: &ThreatInfo) -> ThreatLevel {
        let mut score = threat.severity;

        // Increase threat level based on distance to base (gentle bump to avoid over-escalation).
        if threat.distance_to_base < self.critical_radius {
            score += 0.15;
        } else if threat.distance_to_base < self.alert_radius {
            score += 0.05;
        }

        score = score.clamp(0.0, 1.0);

        if score >= 0.9 {
            ThreatLevel::Critical
        } else if score >= 0.7 {
            ThreatLevel::High
        } else if score >= 0.4 {
            ThreatLevel::Moderate
        } else if score >= 0.1 {
            ThreatLevel::Low
        } else {
            ThreatLevel::None
        }
    }

    fn recalculate_overall_threat(&mut self) {
        if self.active_threats.is_empty() {
            self.overall_threat_level = ThreatLevel::None;
            self.threat_score = 0.0;
            return;
        }

        // Sum all threat severities
        self.threat_score = self
            .active_threats
            .values()
            .map(|t| t.severity)
            .sum::<f32>()
            / self.active_threats.len() as f32;

        // Find highest individual threat level
        let max_threat_level = self
            .active_threats
            .values()
            .map(|t| t.threat_level)
            .max()
            .unwrap_or(ThreatLevel::None);

        self.overall_threat_level = max_threat_level;
    }
}

impl Default for ThreatAssessmentSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_assessment() {
        let mut system = ThreatAssessmentSystem::new();
        system.set_base_position([0.0, 0.0, 0.0].into());

        let threat = ThreatInfo {
            threat_id: 123,
            threat_type: ThreatType::Military,
            threat_level: ThreatLevel::Moderate,
            position: [100.0, 100.0, 0.0].into(),
            severity: 0.7,
            detection_frame: 0,
            last_update_frame: 0,
            estimated_strength: 50.0,
            distance_to_base: 141.42,
        };

        system.add_threat(threat);
        assert_eq!(system.active_threats.len(), 1);
        assert_eq!(system.get_overall_threat_level(), ThreatLevel::High);
    }
}
