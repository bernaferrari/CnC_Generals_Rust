//! Tech Progression Logic System
//!
//! Manages technology research and upgrade progression for AI players:
//! - Tech tree navigation
//! - Research prioritization
//! - Upgrade timing
//! - Tech rush detection and response

use crate::ai::AiError;
use crate::common::ObjectID;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechTier {
    Tier1, // Basic units and structures
    Tier2, // Advanced units
    Tier3, // Elite units and superweapons
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchPriority {
    Critical,
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone)]
pub struct TechNode {
    pub tech_name: String,
    pub tier: TechTier,
    pub priority: ResearchPriority,
    pub prerequisites: Vec<String>,
    pub cost: i32,
    pub research_time: f32,
    pub unlocks: Vec<String>,
}

impl TechNode {
    pub fn new(tech_name: String, tier: TechTier) -> Self {
        Self {
            tech_name,
            tier,
            priority: ResearchPriority::Normal,
            prerequisites: Vec::new(),
            cost: 0,
            research_time: 0.0,
            unlocks: Vec::new(),
        }
    }

    pub fn can_research(&self, completed_research: &HashSet<String>) -> bool {
        self.prerequisites
            .iter()
            .all(|prereq| completed_research.contains(prereq))
    }
}

#[derive(Debug)]
pub struct TechProgressionManager {
    tech_tree: HashMap<String, TechNode>,
    completed_research: HashSet<String>,
    research_queue: Vec<String>,
    current_research: Option<String>,
    current_progress: f32,

    current_tier: TechTier,
    available_resources: i32,

    strategy: TechStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechStrategy {
    Rush,     // Fast tech progression
    Balanced, // Balanced progression
    Economic, // Tech for economy
    Military, // Military tech focus
}

impl TechProgressionManager {
    pub fn new() -> Self {
        Self {
            tech_tree: HashMap::new(),
            completed_research: HashSet::new(),
            research_queue: Vec::new(),
            current_research: None,
            current_progress: 0.0,
            current_tier: TechTier::Tier1,
            available_resources: 0,
            strategy: TechStrategy::Balanced,
        }
    }

    pub fn add_tech_node(&mut self, node: TechNode) {
        self.tech_tree.insert(node.tech_name.clone(), node);
    }

    pub fn set_strategy(&mut self, strategy: TechStrategy) {
        self.strategy = strategy;
        self.recalculate_priorities();
    }

    pub fn can_research(&self, tech_name: &str) -> bool {
        if let Some(node) = self.tech_tree.get(tech_name) {
            node.can_research(&self.completed_research) && node.cost <= self.available_resources
        } else {
            false
        }
    }

    pub fn start_research(&mut self, tech_name: String) -> Result<(), AiError> {
        if !self.can_research(&tech_name) {
            return Err(AiError::InvalidCommand);
        }

        self.current_research = Some(tech_name);
        self.current_progress = 0.0;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), AiError> {
        if let Some(ref tech_name) = self.current_research.clone() {
            if let Some(node) = self.tech_tree.get(tech_name) {
                self.current_progress += delta_time;

                if self.current_progress >= node.research_time {
                    // Research complete
                    self.completed_research.insert(tech_name.clone());
                    self.current_research = None;
                    self.current_progress = 0.0;

                    // Update tier if needed
                    self.update_current_tier();

                    // Auto-start next research if available
                    self.auto_queue_next_research();
                }
            }
        }

        Ok(())
    }

    pub fn queue_research(&mut self, tech_name: String) {
        if !self.research_queue.contains(&tech_name) {
            self.research_queue.push(tech_name);
        }
    }

    pub fn get_available_research(&self) -> Vec<String> {
        self.tech_tree
            .values()
            .filter(|node| {
                !self.completed_research.contains(&node.tech_name)
                    && node.can_research(&self.completed_research)
            })
            .map(|node| node.tech_name.clone())
            .collect()
    }

    pub fn get_recommended_research(&self) -> Option<String> {
        let mut available: Vec<_> = self
            .tech_tree
            .values()
            .filter(|node| {
                !self.completed_research.contains(&node.tech_name)
                    && node.can_research(&self.completed_research)
            })
            .collect();

        available.sort_by_key(|node| node.priority as u8);

        available.first().map(|node| node.tech_name.clone())
    }

    pub fn has_completed(&self, tech_name: &str) -> bool {
        self.completed_research.contains(tech_name)
    }

    pub fn get_current_tier(&self) -> TechTier {
        self.current_tier
    }

    pub fn update_resources(&mut self, resources: i32) {
        self.available_resources = resources;
    }

    fn update_current_tier(&mut self) {
        // Determine current tier based on completed research
        let tier3_count = self
            .completed_research
            .iter()
            .filter(|tech| {
                self.tech_tree
                    .get(*tech)
                    .map(|n| n.tier == TechTier::Tier3)
                    .unwrap_or(false)
            })
            .count();

        let tier2_count = self
            .completed_research
            .iter()
            .filter(|tech| {
                self.tech_tree
                    .get(*tech)
                    .map(|n| n.tier == TechTier::Tier2)
                    .unwrap_or(false)
            })
            .count();

        if tier3_count > 0 {
            self.current_tier = TechTier::Tier3;
        } else if tier2_count > 0 {
            self.current_tier = TechTier::Tier2;
        } else {
            self.current_tier = TechTier::Tier1;
        }
    }

    fn auto_queue_next_research(&mut self) {
        if let Some(next_tech) = self.get_recommended_research() {
            if self.can_research(&next_tech) {
                let _ = self.start_research(next_tech);
            }
        }
    }

    fn recalculate_priorities(&mut self) {
        // Adjust priorities based on strategy
        for node in self.tech_tree.values_mut() {
            node.priority = match self.strategy {
                TechStrategy::Rush => {
                    if node.tech_name.contains("Speed") || node.tech_name.contains("Fast") {
                        ResearchPriority::Critical
                    } else {
                        ResearchPriority::Low
                    }
                }
                TechStrategy::Economic => {
                    if node.tech_name.contains("Economy") || node.tech_name.contains("Resource") {
                        ResearchPriority::High
                    } else {
                        ResearchPriority::Normal
                    }
                }
                TechStrategy::Military => {
                    if node.tech_name.contains("Weapon") || node.tech_name.contains("Armor") {
                        ResearchPriority::High
                    } else {
                        ResearchPriority::Normal
                    }
                }
                TechStrategy::Balanced => ResearchPriority::Normal,
            };
        }
    }

    pub fn initialize_standard_tech_tree(&mut self) {
        // Add standard tech upgrades
        let mut node = TechNode::new("BasicWeapons".to_string(), TechTier::Tier1);
        node.cost = 1000;
        node.research_time = 30.0;
        self.add_tech_node(node);

        let mut node = TechNode::new("AdvancedWeapons".to_string(), TechTier::Tier2);
        node.cost = 2000;
        node.research_time = 45.0;
        node.prerequisites.push("BasicWeapons".to_string());
        self.add_tech_node(node);

        let mut node = TechNode::new("EliteWeapons".to_string(), TechTier::Tier3);
        node.cost = 3000;
        node.research_time = 60.0;
        node.prerequisites.push("AdvancedWeapons".to_string());
        self.add_tech_node(node);
    }
}

impl Default for TechProgressionManager {
    fn default() -> Self {
        let mut manager = Self::new();
        manager.initialize_standard_tech_tree();
        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tech_progression() {
        let mut manager = TechProgressionManager::new();

        let node = TechNode::new("BasicTech".to_string(), TechTier::Tier1);
        manager.add_tech_node(node);

        manager.update_resources(5000);
        assert!(manager.can_research("BasicTech"));
    }

    #[test]
    fn test_tech_prerequisites() {
        let manager = TechProgressionManager::default();
        let available = manager.get_available_research();

        // Should only have tier 1 tech available initially
        assert!(!available.is_empty());
    }
}
