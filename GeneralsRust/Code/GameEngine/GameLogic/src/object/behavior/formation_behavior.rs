//! Formation Behavior Implementation
//!
//! Manages unit formations and coordinated group movement,
//! including formation types, positioning, and cohesion maintenance.

use crate::object::behavior::advanced_behavior_system::BehaviorEvent;

use super::advanced_behavior_system::{
    AdvancedBehavior, BehaviorContext, BehaviorOutcome, BehaviorPriority, BehaviorState,
};
use crate::common::*;
use crate::object::{Object, ObjectId};
use crate::GameLogicResult;
use async_trait::async_trait;
use nalgebra::{Point2, Vector2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Formation types available
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FormationType {
    Line,
    Column,
    Wedge,
    Box,
    Circle,
    Diamond,
    Echelon,
    Custom,
}

/// Formation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationConfig {
    /// Type of formation
    pub formation_type: FormationType,
    /// Spacing between units
    pub unit_spacing: f32,
    /// Maximum units in formation
    pub max_units: usize,
    /// Formation leader selection strategy
    pub leader_strategy: LeaderStrategy,
    /// Whether to maintain formation during movement
    pub maintain_during_movement: bool,
    /// Whether to maintain formation during combat
    pub maintain_during_combat: bool,
    /// Formation cohesion strength (0.0-1.0)
    pub cohesion_strength: f32,
    /// Maximum distance before reforming
    pub max_dispersion_distance: f32,
    /// Time to wait before reforming after dispersion
    pub reform_delay: f32,
    /// Formation rotation speed (radians per second)
    pub rotation_speed: f32,
    /// Custom formation positions (for FormationType::Custom)
    pub custom_positions: Vec<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LeaderStrategy {
    FirstUnit,
    CenterMass,
    HighestRank,
    PlayerDesignated,
}

impl Default for FormationConfig {
    fn default() -> Self {
        Self {
            formation_type: FormationType::Line,
            unit_spacing: 50.0,
            max_units: 12,
            leader_strategy: LeaderStrategy::CenterMass,
            maintain_during_movement: true,
            maintain_during_combat: false,
            cohesion_strength: 0.7,
            max_dispersion_distance: 200.0,
            reform_delay: 2.0,
            rotation_speed: std::f32::consts::PI / 2.0, // 90 degrees per second
            custom_positions: Vec::new(),
        }
    }
}

/// Unit position in formation
#[derive(Debug, Clone)]
pub struct FormationSlot {
    pub unit_id: ObjectId,
    pub assigned_position: Point2<f32>,
    pub slot_index: usize,
    pub last_update: Instant,
}

/// Formation state tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormationState {
    Forming,
    InFormation,
    Dispersed,
    Reforming,
    Broken,
}

/// Formation Behavior for coordinated group movement
#[derive(Debug)]
pub struct FormationBehavior {
    config: FormationConfig,
    formation_state: FormationState,
    formation_center: Point2<f32>,
    formation_angle: f32,
    leader_id: Option<ObjectId>,
    formation_slots: Vec<FormationSlot>,
    last_reform_time: Option<Instant>,
    cohesion_forces: HashMap<ObjectId, Vector2<f32>>,
    target_formation_positions: Vec<Point2<f32>>,
}

impl FormationBehavior {
    pub fn new() -> Self {
        Self::with_config(FormationConfig::default())
    }

    pub fn with_config(config: FormationConfig) -> Self {
        Self {
            config,
            formation_state: FormationState::Forming,
            formation_center: Point2::origin(),
            formation_angle: 0.0,
            leader_id: None,
            formation_slots: Vec::new(),
            last_reform_time: None,
            cohesion_forces: HashMap::new(),
            target_formation_positions: Vec::new(),
        }
    }

    pub fn add_unit(&mut self, unit_id: ObjectId) -> Result<bool, String> {
        if self.formation_slots.len() >= self.config.max_units {
            return Ok(false);
        }

        if self
            .formation_slots
            .iter()
            .any(|slot| slot.unit_id == unit_id)
        {
            return Ok(false);
        }

        let slot_index = self.formation_slots.len();
        let assigned_position = self.calculate_slot_position(slot_index);

        let slot = FormationSlot {
            unit_id,
            assigned_position,
            slot_index,
            last_update: Instant::now(),
        };

        self.formation_slots.push(slot);
        self.update_formation_positions();

        Ok(true)
    }

    pub fn remove_unit(&mut self, unit_id: ObjectId) -> bool {
        if let Some(pos) = self
            .formation_slots
            .iter()
            .position(|slot| slot.unit_id == unit_id)
        {
            self.formation_slots.remove(pos);
            for (i, slot) in self.formation_slots.iter_mut().enumerate() {
                slot.slot_index = i;
            }
            self.update_formation_positions();
            true
        } else {
            false
        }
    }

    fn calculate_slot_position(&self, slot_index: usize) -> Point2<f32> {
        let spacing = self.config.unit_spacing;

        match self.config.formation_type {
            FormationType::Line => {
                let offset_x =
                    (slot_index as f32 - (self.formation_slots.len() as f32 - 1.0) / 2.0) * spacing;
                Point2::new(offset_x, 0.0)
            }
            FormationType::Column => Point2::new(0.0, -(slot_index as f32) * spacing),
            FormationType::Wedge => {
                let row = ((2.0 * slot_index as f32 + 0.25).sqrt() - 0.5).floor() as usize;
                let col = slot_index - (row * (row + 1)) / 2;
                let offset_x = (col as f32 - row as f32 / 2.0) * spacing;
                let offset_y = -(row as f32) * spacing * 0.866;
                Point2::new(offset_x, offset_y)
            }
            FormationType::Box => {
                let side_length = (self.config.max_units as f32).sqrt().ceil() as usize;
                let row = slot_index / side_length;
                let col = slot_index % side_length;
                let offset_x = (col as f32 - (side_length as f32 - 1.0) / 2.0) * spacing;
                let offset_y = (row as f32 - (side_length as f32 - 1.0) / 2.0) * spacing;
                Point2::new(offset_x, offset_y)
            }
            FormationType::Circle => {
                if slot_index == 0 {
                    Point2::origin()
                } else {
                    let angle = (slot_index - 1) as f32 * 2.0 * std::f32::consts::PI
                        / (self.formation_slots.len() - 1) as f32;
                    let radius = spacing * 2.0;
                    Point2::new(radius * angle.cos(), radius * angle.sin())
                }
            }
            FormationType::Diamond => match slot_index {
                0 => Point2::new(0.0, spacing),
                1 => Point2::new(-spacing, 0.0),
                2 => Point2::new(spacing, 0.0),
                3 => Point2::new(0.0, -spacing),
                _ => {
                    let extended_slot = slot_index - 4;
                    let extended_spacing = spacing * 2.0;
                    let angle = extended_slot as f32 * std::f32::consts::PI / 2.0;
                    Point2::new(
                        extended_spacing * angle.cos(),
                        extended_spacing * angle.sin(),
                    )
                }
            },
            FormationType::Echelon => {
                let offset_x = slot_index as f32 * spacing * 0.5;
                let offset_y = -(slot_index as f32) * spacing * 0.866;
                Point2::new(offset_x, offset_y)
            }
            FormationType::Custom => {
                if slot_index < self.config.custom_positions.len() {
                    let pos = self.config.custom_positions[slot_index];
                    Point2::new(pos.0, pos.1)
                } else {
                    let offset_x = slot_index as f32 * spacing;
                    Point2::new(offset_x, 0.0)
                }
            }
        }
    }

    fn update_formation_positions(&mut self) {
        self.target_formation_positions.clear();
        for (i, _slot) in self.formation_slots.iter().enumerate() {
            let local_pos = self.calculate_slot_position(i);
            let cos_angle = self.formation_angle.cos();
            let sin_angle = self.formation_angle.sin();
            let rotated_x = local_pos.x * cos_angle - local_pos.y * sin_angle;
            let rotated_y = local_pos.x * sin_angle + local_pos.y * cos_angle;
            let world_pos = Point2::new(
                self.formation_center.x + rotated_x,
                self.formation_center.y + rotated_y,
            );
            self.target_formation_positions.push(world_pos);
        }
    }

    async fn update_formation_center(&mut self, objects: &[&Object]) -> GameLogicResult<()> {
        let obj_map: std::collections::HashMap<ObjectId, &Object> =
            objects.iter().map(|obj| (obj.get_id(), *obj)).collect();

        match self.config.leader_strategy {
            LeaderStrategy::FirstUnit => {
                if let Some(first_slot) = self.formation_slots.first() {
                    if let Some(leader_obj) = obj_map.get(&first_slot.unit_id) {
                        let pos = leader_obj.get_position();
                        self.formation_center = Point2::new(pos.x, pos.y);
                        self.leader_id = Some(first_slot.unit_id);
                    }
                }
            }
            LeaderStrategy::CenterMass => {
                let mut sum_x = 0.0;
                let mut sum_y = 0.0;
                let mut count = 0;
                for slot in &self.formation_slots {
                    if let Some(obj) = obj_map.get(&slot.unit_id) {
                        let pos = obj.get_position();
                        sum_x += pos.x;
                        sum_y += pos.y;
                        count += 1;
                    }
                }
                if count > 0 {
                    self.formation_center = Point2::new(sum_x / count as f32, sum_y / count as f32);
                    self.leader_id = None;
                }
            }
            LeaderStrategy::HighestRank => {
                let mut highest_rank_obj: Option<&Object> = None;
                let mut highest_rank = -1;
                for slot in &self.formation_slots {
                    if let Some(obj) = obj_map.get(&slot.unit_id) {
                        let rank = obj.get_veterancy_level() as i32;
                        if rank > highest_rank {
                            highest_rank = rank;
                            highest_rank_obj = Some(*obj);
                        }
                    }
                }
                if let Some(leader_obj) = highest_rank_obj {
                    let pos = leader_obj.get_position();
                    self.formation_center = Point2::new(pos.x, pos.y);
                    self.leader_id = Some(leader_obj.get_id());
                }
            }
            LeaderStrategy::PlayerDesignated => {
                if let Some(leader_id) = self.leader_id {
                    if let Some(leader_obj) = obj_map.get(&leader_id) {
                        let pos = leader_obj.get_position();
                        self.formation_center = Point2::new(pos.x, pos.y);
                    }
                }
            }
        }
        Ok(())
    }

    fn calculate_cohesion_forces(&mut self, objects: &[&Object]) {
        let obj_map: std::collections::HashMap<ObjectId, &Object> =
            objects.iter().map(|obj| (obj.get_id(), *obj)).collect();

        self.cohesion_forces.clear();
        for (i, slot) in self.formation_slots.iter().enumerate() {
            if let Some(obj) = obj_map.get(&slot.unit_id) {
                let current_pos = obj.get_position();
                let target_pos = self
                    .target_formation_positions
                    .get(i)
                    .unwrap_or(&Point2::origin());

                let force_vector =
                    Vector2::new(target_pos.x - current_pos.x, target_pos.y - current_pos.y);

                let force_magnitude = force_vector.length() * self.config.cohesion_strength;
                let normalized_force = if force_magnitude > 0.0 {
                    force_vector.normalize() * force_magnitude
                } else {
                    Vector2::zeros()
                };

                self.cohesion_forces.insert(slot.unit_id, normalized_force);
            }
        }
    }

    fn is_formation_dispersed(&self, objects: &[&Object]) -> bool {
        let obj_map: std::collections::HashMap<ObjectId, &Object> =
            objects.iter().map(|obj| (obj.get_id(), *obj)).collect();

        let mut max_distance = 0.0;
        for slot in &self.formation_slots {
            if let Some(obj) = obj_map.get(&slot.unit_id) {
                let pos = obj.get_position();
                let distance = ((pos.x - self.formation_center.x).powi(2)
                    + (pos.y - self.formation_center.y).powi(2))
                .sqrt();
                max_distance = max_distance.max(distance);
            }
        }
        max_distance > self.config.max_dispersion_distance
    }

    async fn apply_cohesion_forces(&self, objects: &mut [&mut Object]) -> GameLogicResult<()> {
        for obj in objects.iter_mut() {
            if let Some(force) = self.cohesion_forces.get(&obj.get_id()) {
                if force.norm() > 0.1 {
                    obj.apply_movement_force(force.x, force.y, 0.0).await?;
                }
            }
        }
        Ok(())
    }

    pub fn set_leader(&mut self, leader_id: ObjectId) -> bool {
        if self
            .formation_slots
            .iter()
            .any(|slot| slot.unit_id == leader_id)
        {
            self.leader_id = Some(leader_id);
            true
        } else {
            false
        }
    }

    pub fn break_formation(&mut self) {
        self.formation_state = FormationState::Broken;
        self.cohesion_forces.clear();
    }

    pub fn reform_formation(&mut self) {
        self.formation_state = FormationState::Reforming;
        self.last_reform_time = Some(Instant::now());
    }
}

#[async_trait]
impl AdvancedBehavior for FormationBehavior {
    fn name(&self) -> &str {
        "Formation"
    }

    fn priority(&self) -> BehaviorPriority {
        BehaviorPriority::High
    }

    async fn initialize(
        &mut self,
        object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        let object_id = object.get_id();
        if !self
            .formation_slots
            .iter()
            .any(|slot| slot.unit_id == object_id)
        {
            self.add_unit(object_id).map_err(|e| {
                crate::GameLogicError::Configuration(format!("Failed to add unit: {}", e))
            })?;
        }
        log::info!(
            "Formation behavior initialized: object_id={}, formation_type={:?}, max_units={}",
            object.get_id(),
            self.config.formation_type,
            self.config.max_units
        );
        Ok(())
    }

    async fn update(
        &mut self,
        object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<BehaviorOutcome> {
        let formation_objects: Vec<&Object> = vec![object];
        let mut formation_objects_mut: Vec<&mut Object> = vec![object];

        self.update_formation_center(&formation_objects).await?;
        self.update_formation_positions();

        match self.formation_state {
            FormationState::Forming => {
                let mut all_in_position = true;
                for (i, slot) in self.formation_slots.iter().enumerate() {
                    if let Some(target_pos) = self.target_formation_positions.get(i) {
                        if let Some(obj) = formation_objects
                            .iter()
                            .find(|o| o.get_id() == slot.unit_id)
                        {
                            let pos = obj.get_position();
                            let distance = ((pos.x - target_pos.x).powi(2)
                                + (pos.y - target_pos.y).powi(2))
                            .sqrt();
                            if distance > self.config.unit_spacing * 0.5 {
                                all_in_position = false;
                                break;
                            }
                        }
                    }
                }
                if all_in_position {
                    self.formation_state = FormationState::InFormation;
                    log::debug!("Formation established");
                }
            }
            FormationState::InFormation => {
                if self.is_formation_dispersed(&formation_objects) {
                    self.formation_state = FormationState::Dispersed;
                    log::debug!("Formation dispersed");
                }
                if self.config.maintain_during_movement || self.config.maintain_during_combat {
                    self.calculate_cohesion_forces(&formation_objects);
                    self.apply_cohesion_forces(&mut formation_objects_mut)
                        .await?;
                }
            }
            FormationState::Dispersed => {
                if let Some(last_reform) = self.last_reform_time {
                    if last_reform.elapsed() >= Duration::from_secs_f32(self.config.reform_delay) {
                        self.formation_state = FormationState::Reforming;
                        log::debug!("Starting formation reform");
                    }
                } else {
                    self.last_reform_time = Some(Instant::now());
                }
            }
            FormationState::Reforming => {
                self.calculate_cohesion_forces(&formation_objects);
                self.apply_cohesion_forces(&mut formation_objects_mut)
                    .await?;
                if !self.is_formation_dispersed(&formation_objects) {
                    self.formation_state = FormationState::InFormation;
                    log::debug!("Formation reformed");
                }
            }
            FormationState::Broken => return Ok(BehaviorOutcome::Continue),
        }
        Ok(BehaviorOutcome::Continue)
    }

    async fn cleanup(
        &mut self,
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        self.formation_slots.clear();
        self.cohesion_forces.clear();
        self.formation_state = FormationState::Broken;
        log::info!("Formation behavior cleanup completed");
        Ok(())
    }

    async fn handle_event(
        &mut self,
        _event: &BehaviorEvent, // Would need to parse event data
        _object: &mut Object,
        _context: &BehaviorContext,
    ) -> GameLogicResult<()> {
        // Simplified event handling - requires parsing data which is hard in this generic context
        // Leaving implementation empty for now to satisfy trait
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formation_creation() {
        let behavior = FormationBehavior::new();
        assert_eq!(behavior.formation_state, FormationState::Forming);
        assert_eq!(behavior.config.formation_type, FormationType::Line);
    }
    // ...
}
