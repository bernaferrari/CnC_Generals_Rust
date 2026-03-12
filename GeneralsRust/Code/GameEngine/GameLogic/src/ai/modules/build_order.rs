//! Build Order Optimization System
//!
//! Manages and optimizes building construction orders for AI players:
//! - Build order planning
//! - Resource optimization
//! - Prerequisite management
//! - Dynamic adaptation based on game state

use crate::ai::AiError;
use crate::common::{ObjectID, Real};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildPriority {
    Critical, // Must build immediately
    High,     // Build as soon as possible
    Normal,   // Standard priority
    Low,      // Build when resources available
    Optional, // Nice to have
}

#[derive(Debug, Clone)]
pub struct BuildOrder {
    pub building_type: String,
    pub priority: BuildPriority,
    pub prerequisites: Vec<String>,
    pub cost: i32,
    pub build_time: Real,
    pub max_count: Option<usize>,
    pub current_count: usize,
}

impl BuildOrder {
    pub fn new(building_type: String, priority: BuildPriority) -> Self {
        Self {
            building_type,
            priority,
            prerequisites: Vec::new(),
            cost: 0,
            build_time: 0.0,
            max_count: None,
            current_count: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        if let Some(max) = self.max_count {
            self.current_count >= max
        } else {
            false
        }
    }

    pub fn can_build(&self, available_buildings: &HashMap<String, usize>) -> bool {
        if self.is_complete() {
            return false;
        }

        for prereq in &self.prerequisites {
            if !available_buildings.contains_key(prereq) {
                return false;
            }
        }

        true
    }
}

#[derive(Debug)]
pub struct BuildOrderOptimizer {
    build_queue: VecDeque<BuildOrder>,
    completed_orders: Vec<BuildOrder>,
    current_buildings: HashMap<String, usize>,
    available_resources: i32,

    strategy_template: HashMap<String, BuildOrder>,
}

impl BuildOrderOptimizer {
    pub fn new() -> Self {
        Self {
            build_queue: VecDeque::new(),
            completed_orders: Vec::new(),
            current_buildings: HashMap::new(),
            available_resources: 0,
            strategy_template: HashMap::new(),
        }
    }

    pub fn add_build_order(&mut self, order: BuildOrder) {
        // Insert in priority order
        let insert_pos = self
            .build_queue
            .iter()
            .position(|o| o.priority as u8 > order.priority as u8)
            .unwrap_or(self.build_queue.len());

        self.build_queue.insert(insert_pos, order);
    }

    pub fn get_next_build(&mut self) -> Option<BuildOrder> {
        // Find first buildable order
        let buildable_idx = self.build_queue.iter().position(|order| {
            order.can_build(&self.current_buildings) && order.cost <= self.available_resources
        });

        buildable_idx.and_then(|idx| self.build_queue.remove(idx))
    }

    pub fn update_resources(&mut self, resources: i32) {
        self.available_resources = resources;
    }

    pub fn on_building_complete(&mut self, building_type: String) {
        *self.current_buildings.entry(building_type).or_insert(0) += 1;
    }

    pub fn set_strategy_template(&mut self, strategy: &str) {
        self.strategy_template.clear();

        match strategy {
            "rush" => self.setup_rush_build_order(),
            "economic" => self.setup_economic_build_order(),
            "defensive" => self.setup_defensive_build_order(),
            "balanced" => self.setup_balanced_build_order(),
            _ => self.setup_balanced_build_order(),
        }
    }

    fn setup_rush_build_order(&mut self) {
        // Prioritize military production
        let mut order = BuildOrder::new("Barracks".to_string(), BuildPriority::Critical);
        order.max_count = Some(1);
        self.add_build_order(order);

        let mut order = BuildOrder::new("SupplyCenter".to_string(), BuildPriority::High);
        order.max_count = Some(1);
        self.add_build_order(order);

        let mut order = BuildOrder::new("Barracks".to_string(), BuildPriority::High);
        order.max_count = Some(2);
        self.add_build_order(order);
    }

    fn setup_economic_build_order(&mut self) {
        // Prioritize economy
        let mut order = BuildOrder::new("SupplyCenter".to_string(), BuildPriority::Critical);
        order.max_count = Some(2);
        self.add_build_order(order);

        let mut order = BuildOrder::new("PowerPlant".to_string(), BuildPriority::High);
        order.max_count = Some(2);
        self.add_build_order(order);
    }

    fn setup_defensive_build_order(&mut self) {
        // Prioritize defenses
        let mut order = BuildOrder::new("SupplyCenter".to_string(), BuildPriority::High);
        order.max_count = Some(1);
        self.add_build_order(order);

        let mut order = BuildOrder::new("Defense".to_string(), BuildPriority::Critical);
        order.max_count = Some(4);
        self.add_build_order(order);
    }

    fn setup_balanced_build_order(&mut self) {
        // Balanced approach
        let mut order = BuildOrder::new("PowerPlant".to_string(), BuildPriority::High);
        order.max_count = Some(1);
        self.add_build_order(order);

        let mut order = BuildOrder::new("SupplyCenter".to_string(), BuildPriority::High);
        order.max_count = Some(1);
        self.add_build_order(order);

        let mut order = BuildOrder::new("Barracks".to_string(), BuildPriority::Normal);
        order.max_count = Some(1);
        self.add_build_order(order);
    }

    pub fn optimize_build_order(&mut self) {
        // Re-prioritize based on current game state
        let mut queue: Vec<BuildOrder> = self.build_queue.drain(..).collect();
        queue.sort_by_key(|order| order.priority as u8);
        self.build_queue = queue.into();
    }

    pub fn get_build_queue_size(&self) -> usize {
        self.build_queue.len()
    }
}

impl Default for BuildOrderOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_order() {
        let mut order = BuildOrder::new("Barracks".to_string(), BuildPriority::High);
        order.max_count = Some(2);

        assert!(!order.is_complete());
        order.current_count = 2;
        assert!(order.is_complete());
    }

    #[test]
    fn test_build_optimizer() {
        let mut optimizer = BuildOrderOptimizer::new();
        optimizer.set_strategy_template("rush");

        assert!(optimizer.get_build_queue_size() > 0);
    }
}
