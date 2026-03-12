////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Formation System - Unit formation and group movement
//!
//! This module provides the formation system for managing unit formations,
//! group movement patterns, and coordinated unit behavior.
//! Matches C++ TensileFormationUpdate and formation command processing.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

use super::command::{Command, CommandType};
use super::rts_command::RtsCommand;
use crate::common::{AsciiString, Bool, Coord3D, Int, ObjectID, Real, UnsignedInt};

/// Formation types supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormationType {
    /// No formation - units move independently
    None,
    /// Line formation - units form a line
    Line,
    /// Column formation - units form a column
    Column,
    /// Box formation - units form a rectangular box
    Box,
    /// Wedge formation - units form a V shape
    Wedge,
    /// Circle formation - units form a circle
    Circle,
    /// Custom formation defined by template
    Custom(u32),
}

impl Default for FormationType {
    fn default() -> Self {
        FormationType::None
    }
}

/// Formation behavior settings
#[derive(Debug, Clone)]
pub struct FormationSettings {
    /// Formation type
    pub formation_type: FormationType,

    /// Spacing between units in formation
    pub unit_spacing: Real,

    /// Formation orientation in radians
    pub orientation: Real,

    /// Whether to maintain formation during movement
    pub maintain_during_movement: bool,

    /// Whether to maintain formation during combat
    pub maintain_during_combat: bool,

    /// Formation stiffness (how strictly units adhere to formation)
    pub stiffness: Real, // 0.0 = loose, 1.0 = rigid

    /// Maximum distance a unit can deviate from formation position
    pub max_deviation: Real,

    /// Speed of formation reformation
    pub reformation_speed: Real,
}

impl Default for FormationSettings {
    fn default() -> Self {
        Self {
            formation_type: FormationType::None,
            unit_spacing: 50.0, // 50 units apart
            orientation: 0.0,   // Facing north
            maintain_during_movement: true,
            maintain_during_combat: false,
            stiffness: 0.7,         // Moderately strict
            max_deviation: 75.0,    // Allow some flexibility
            reformation_speed: 0.1, // Gradual reformation
        }
    }
}

/// A single position within a formation
#[derive(Debug, Clone)]
pub struct FormationPosition {
    /// Relative position offset from formation center
    pub relative_position: Coord3D,

    /// Priority of this position (0 = leader, higher = follower)
    pub priority: u32,

    /// Desired facing direction relative to formation orientation
    pub facing_offset: Real,

    /// Object currently assigned to this position
    pub assigned_object: Option<ObjectID>,

    /// Whether this position is currently occupied
    pub is_occupied: bool,
}

impl FormationPosition {
    pub fn new(relative_pos: Coord3D, priority: u32) -> Self {
        Self {
            relative_position: relative_pos,
            priority,
            facing_offset: 0.0,
            assigned_object: None,
            is_occupied: false,
        }
    }
}

/// Formation template that defines the shape and positions
#[derive(Debug, Clone)]
pub struct FormationTemplate {
    /// Template name
    pub name: AsciiString,

    /// Formation type
    pub formation_type: FormationType,

    /// Predefined positions in the formation
    pub positions: Vec<FormationPosition>,

    /// Default settings for this formation
    pub default_settings: FormationSettings,

    /// Maximum units this formation can accommodate
    pub max_units: usize,

    /// Minimum units required for this formation to be effective
    pub min_units: usize,
}

impl FormationTemplate {
    /// Create a line formation template
    pub fn create_line_formation(unit_count: usize, spacing: Real) -> Self {
        let mut positions = Vec::new();
        let half_width = (unit_count as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let x_offset = i as Real * spacing - half_width;
            positions.push(FormationPosition::new(
                Coord3D::new(x_offset, 0.0, 0.0),
                i as u32,
            ));
        }

        let mut settings = FormationSettings::default();
        settings.formation_type = FormationType::Line;
        settings.unit_spacing = spacing;

        Self {
            name: AsciiString::from("Line"),
            formation_type: FormationType::Line,
            positions,
            default_settings: settings,
            max_units: unit_count,
            min_units: 2,
        }
    }

    /// Create a column formation template
    pub fn create_column_formation(unit_count: usize, spacing: Real) -> Self {
        let mut positions = Vec::new();
        let half_depth = (unit_count as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let y_offset = i as Real * spacing - half_depth;
            positions.push(FormationPosition::new(
                Coord3D::new(0.0, y_offset, 0.0),
                i as u32,
            ));
        }

        let mut settings = FormationSettings::default();
        settings.formation_type = FormationType::Column;
        settings.unit_spacing = spacing;

        Self {
            name: AsciiString::from("Column"),
            formation_type: FormationType::Column,
            positions,
            default_settings: settings,
            max_units: unit_count,
            min_units: 2,
        }
    }

    /// Create a box formation template
    pub fn create_box_formation(width: usize, height: usize, spacing: Real) -> Self {
        let mut positions = Vec::new();
        let half_width = (width as Real - 1.0) * spacing * 0.5;
        let half_height = (height as Real - 1.0) * spacing * 0.5;

        let mut priority = 0;
        for row in 0..height {
            for col in 0..width {
                let x_offset = col as Real * spacing - half_width;
                let y_offset = row as Real * spacing - half_height;
                positions.push(FormationPosition::new(
                    Coord3D::new(x_offset, y_offset, 0.0),
                    priority,
                ));
                priority += 1;
            }
        }

        let mut settings = FormationSettings::default();
        settings.formation_type = FormationType::Box;
        settings.unit_spacing = spacing;

        Self {
            name: AsciiString::from("Box"),
            formation_type: FormationType::Box,
            positions,
            default_settings: settings,
            max_units: width * height,
            min_units: 4,
        }
    }

    /// Create a wedge formation template
    pub fn create_wedge_formation(unit_count: usize, spacing: Real) -> Self {
        let mut positions = Vec::new();

        // Leader at the front
        positions.push(FormationPosition::new(Coord3D::new(0.0, 0.0, 0.0), 0));

        // Create V-shape behind leader
        let mut current_row = 1;
        let mut units_placed = 1;

        while units_placed < unit_count && current_row < 10 {
            let units_in_row = (current_row * 2).min(unit_count - units_placed);
            let row_y = -(current_row as Real) * spacing;

            for i in 0..units_in_row {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let x_offset = side * ((i / 2 + 1) as Real) * spacing;

                positions.push(FormationPosition::new(
                    Coord3D::new(x_offset, row_y, 0.0),
                    current_row as u32,
                ));
                units_placed += 1;

                if units_placed >= unit_count {
                    break;
                }
            }

            current_row += 1;
        }

        let mut settings = FormationSettings::default();
        settings.formation_type = FormationType::Wedge;
        settings.unit_spacing = spacing;

        Self {
            name: AsciiString::from("Wedge"),
            formation_type: FormationType::Wedge,
            positions,
            default_settings: settings,
            max_units: unit_count,
            min_units: 3,
        }
    }

    /// Create a circle formation template
    pub fn create_circle_formation(unit_count: usize, radius: Real) -> Self {
        let mut positions = Vec::new();

        for i in 0..unit_count {
            let angle = 2.0 * PI * (i as Real) / (unit_count as Real);
            let x_offset = radius * angle.cos();
            let y_offset = radius * angle.sin();

            let mut position =
                FormationPosition::new(Coord3D::new(x_offset, y_offset, 0.0), i as u32);
            position.facing_offset = angle; // Face outward from circle

            positions.push(position);
        }

        let mut settings = FormationSettings::default();
        settings.formation_type = FormationType::Circle;
        settings.unit_spacing = 2.0 * PI * radius / unit_count as Real;

        Self {
            name: AsciiString::from("Circle"),
            formation_type: FormationType::Circle,
            positions,
            default_settings: settings,
            max_units: unit_count,
            min_units: 3,
        }
    }

    /// Scale formation to accommodate different unit counts
    pub fn scale_for_unit_count(&self, unit_count: usize) -> FormationTemplate {
        if unit_count <= self.positions.len() {
            // Use subset of positions
            let mut scaled = self.clone();
            scaled.positions.truncate(unit_count);
            scaled.max_units = unit_count;
            return scaled;
        }

        // Need to expand formation - create new template based on type
        match self.formation_type {
            FormationType::Line => FormationTemplate::create_line_formation(
                unit_count,
                self.default_settings.unit_spacing,
            ),
            FormationType::Column => FormationTemplate::create_column_formation(
                unit_count,
                self.default_settings.unit_spacing,
            ),
            FormationType::Box => {
                // Create roughly square box
                let side_length = (unit_count as f32).sqrt().ceil() as usize;
                FormationTemplate::create_box_formation(
                    side_length,
                    side_length,
                    self.default_settings.unit_spacing,
                )
            }
            FormationType::Wedge => FormationTemplate::create_wedge_formation(
                unit_count,
                self.default_settings.unit_spacing,
            ),
            FormationType::Circle => {
                let radius = self.default_settings.unit_spacing * unit_count as Real / (2.0 * PI);
                FormationTemplate::create_circle_formation(unit_count, radius)
            }
            _ => self.clone(),
        }
    }
}

/// Active formation instance
#[derive(Debug, Clone)]
pub struct Formation {
    /// Unique formation ID
    pub id: UnsignedInt,

    /// Formation template being used
    pub template: FormationTemplate,

    /// Current settings
    pub settings: FormationSettings,

    /// Objects in this formation
    pub objects: Vec<ObjectID>,

    /// Current formation center position
    pub center_position: Coord3D,

    /// Current formation orientation
    pub orientation: Real,

    /// Movement target (if formation is moving)
    pub movement_target: Option<Coord3D>,

    /// Formation state
    pub state: FormationState,

    /// Frame when formation was created
    pub created_frame: UnsignedInt,

    /// Frame when formation was last updated
    pub last_update_frame: UnsignedInt,

    /// Player who owns this formation
    pub owner_player: Int,
}

/// Formation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationState {
    /// Formation is being created/organized
    Forming,
    /// Formation is maintaining position
    Holding,
    /// Formation is moving to target
    Moving,
    /// Formation is in combat
    Combat,
    /// Formation is disbanding
    Disbanding,
}

impl Formation {
    /// Create new formation
    pub fn new(
        id: UnsignedInt,
        template: FormationTemplate,
        objects: Vec<ObjectID>,
        owner: Int,
        frame: UnsignedInt,
    ) -> Self {
        let mut formation = Self {
            id,
            template: template.clone(),
            settings: template.default_settings.clone(),
            objects,
            center_position: Coord3D::new(0.0, 0.0, 0.0),
            orientation: 0.0,
            movement_target: None,
            state: FormationState::Forming,
            created_frame: frame,
            last_update_frame: frame,
            owner_player: owner,
        };

        // Scale template if needed
        if formation.objects.len() != formation.template.max_units {
            formation.template = formation
                .template
                .scale_for_unit_count(formation.objects.len());
        }

        formation
    }

    /// Assign objects to formation positions
    pub fn assign_positions(&mut self, object_lookup: &dyn FormationObjectLookup) {
        // Clear existing assignments
        for position in &mut self.template.positions {
            position.assigned_object = None;
            position.is_occupied = false;
        }

        // Sort objects by distance from formation center
        let mut object_distances: Vec<(ObjectID, Real)> = self
            .objects
            .iter()
            .filter_map(|&obj_id| {
                object_lookup.get_object_position(obj_id).map(|pos| {
                    let distance = self.distance_3d(pos, self.center_position);
                    (obj_id, distance)
                })
            })
            .collect();

        object_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Assign objects to positions by priority
        let mut position_indices: Vec<usize> = (0..self.template.positions.len()).collect();
        position_indices.sort_by_key(|&i| self.template.positions[i].priority);

        let mut object_index = 0;
        for &pos_index in &position_indices {
            if object_index < object_distances.len() {
                let object_id = object_distances[object_index].0;
                self.template.positions[pos_index].assigned_object = Some(object_id);
                self.template.positions[pos_index].is_occupied = true;
                object_index += 1;
            }
        }
    }

    /// Get desired position for an object in the formation
    pub fn get_desired_position(&self, object_id: ObjectID) -> Option<Coord3D> {
        for position in &self.template.positions {
            if position.assigned_object == Some(object_id) {
                return Some(self.transform_relative_position(position.relative_position));
            }
        }
        None
    }

    /// Transform relative position to world position
    fn transform_relative_position(&self, relative_pos: Coord3D) -> Coord3D {
        let cos_theta = self.orientation.cos();
        let sin_theta = self.orientation.sin();

        // Rotate relative position by formation orientation
        let rotated_x = relative_pos.x * cos_theta - relative_pos.y * sin_theta;
        let rotated_y = relative_pos.x * sin_theta + relative_pos.y * cos_theta;

        // Add to formation center
        Coord3D::new(
            self.center_position.x + rotated_x,
            self.center_position.y + rotated_y,
            self.center_position.z + relative_pos.z,
        )
    }

    /// Update formation center based on object positions
    pub fn update_center(&mut self, object_lookup: &dyn FormationObjectLookup) {
        let mut total_pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut count = 0;

        for &object_id in &self.objects {
            if let Some(pos) = object_lookup.get_object_position(object_id) {
                total_pos.x += pos.x;
                total_pos.y += pos.y;
                total_pos.z += pos.z;
                count += 1;
            }
        }

        if count > 0 {
            let count_real = count as Real;
            self.center_position = Coord3D::new(
                total_pos.x / count_real,
                total_pos.y / count_real,
                total_pos.z / count_real,
            );
        }
    }

    /// Set movement target for formation
    pub fn set_movement_target(&mut self, target: Coord3D) {
        self.movement_target = Some(target);
        self.state = FormationState::Moving;
    }

    /// Check if formation has reached its movement target
    pub fn has_reached_target(&self) -> bool {
        if let Some(target) = self.movement_target {
            let distance = self.distance_3d(self.center_position, target);
            distance < self.settings.unit_spacing // Within one unit spacing
        } else {
            true // No target means we're "there"
        }
    }

    /// Remove object from formation
    pub fn remove_object(&mut self, object_id: ObjectID) -> bool {
        if let Some(index) = self.objects.iter().position(|&id| id == object_id) {
            self.objects.remove(index);

            // Clear from position assignments
            for position in &mut self.template.positions {
                if position.assigned_object == Some(object_id) {
                    position.assigned_object = None;
                    position.is_occupied = false;
                    break;
                }
            }

            return true;
        }
        false
    }

    /// Add object to formation
    pub fn add_object(&mut self, object_id: ObjectID) -> bool {
        if !self.objects.contains(&object_id) && self.objects.len() < self.template.max_units {
            self.objects.push(object_id);
            return true;
        }
        false
    }

    /// Check if formation is still viable
    pub fn is_viable(&self) -> bool {
        !self.objects.is_empty() && self.objects.len() >= self.template.min_units
    }

    /// Get formation movement orders for each object
    pub fn get_movement_orders(
        &self,
        object_lookup: &dyn FormationObjectLookup,
    ) -> Vec<FormationMovementOrder> {
        let mut orders = Vec::new();

        for position in &self.template.positions {
            if let Some(object_id) = position.assigned_object {
                if let Some(current_pos) = object_lookup.get_object_position(object_id) {
                    let desired_pos = self.transform_relative_position(position.relative_position);
                    let distance_from_position = self.distance_3d(current_pos, desired_pos);

                    // Only issue movement order if object is far from desired position
                    if distance_from_position > self.settings.max_deviation {
                        let desired_facing = self.orientation + position.facing_offset;

                        orders.push(FormationMovementOrder {
                            object_id,
                            target_position: desired_pos,
                            target_facing: desired_facing,
                            priority: position.priority,
                            formation_speed: self.calculate_formation_speed(object_lookup),
                        });
                    }
                }
            }
        }

        orders
    }

    /// Calculate appropriate speed for formation movement
    fn calculate_formation_speed(&self, object_lookup: &dyn FormationObjectLookup) -> Real {
        let mut slowest = None;
        for position in &self.template.positions {
            let Some(object_id) = position.assigned_object else {
                continue;
            };
            if !object_lookup.can_object_move(object_id) {
                continue;
            }
            if let Some(speed) = object_lookup.get_object_speed(object_id) {
                slowest = Some(slowest.map_or(speed, |cur: Real| cur.min(speed)));
            }
        }
        slowest.unwrap_or(1.0)
    }

    /// Calculate 3D distance between two points
    fn distance_3d(&self, pos1: Coord3D, pos2: Coord3D) -> Real {
        let dx = pos1.x - pos2.x;
        let dy = pos1.y - pos2.y;
        let dz = pos1.z - pos2.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Movement order for an object in formation
#[derive(Debug, Clone)]
pub struct FormationMovementOrder {
    pub object_id: ObjectID,
    pub target_position: Coord3D,
    pub target_facing: Real,
    pub priority: u32,
    pub formation_speed: Real,
}

/// Trait for formation object lookup
pub trait FormationObjectLookup: Send + Sync {
    fn get_object_position(&self, id: ObjectID) -> Option<Coord3D>;
    fn is_object_alive(&self, id: ObjectID) -> bool;
    fn get_object_speed(&self, id: ObjectID) -> Option<Real>;
    fn can_object_move(&self, id: ObjectID) -> bool;
}

/// Formation manager - manages all active formations
pub struct FormationManager {
    /// Active formations by ID
    formations: HashMap<UnsignedInt, Formation>,

    /// Formation templates
    templates: HashMap<AsciiString, FormationTemplate>,

    /// Next formation ID
    next_formation_id: UnsignedInt,

    /// Object lookup interface
    object_lookup: Option<Arc<RwLock<dyn FormationObjectLookup>>>,

    /// Current frame
    current_frame: UnsignedInt,
}

impl FormationManager {
    /// Create new formation manager
    pub fn new() -> Self {
        let mut manager = Self {
            formations: HashMap::new(),
            templates: HashMap::new(),
            next_formation_id: 1,
            object_lookup: None,
            current_frame: 0,
        };

        // Register default formation templates
        manager.register_default_templates();
        manager
    }

    /// Set object lookup interface
    pub fn set_object_lookup(&mut self, lookup: Arc<RwLock<dyn FormationObjectLookup>>) {
        self.object_lookup = Some(lookup);
    }

    /// Register default formation templates
    fn register_default_templates(&mut self) {
        self.templates.insert(
            AsciiString::from("Line"),
            FormationTemplate::create_line_formation(10, 50.0),
        );
        self.templates.insert(
            AsciiString::from("Column"),
            FormationTemplate::create_column_formation(10, 50.0),
        );
        self.templates.insert(
            AsciiString::from("Box"),
            FormationTemplate::create_box_formation(4, 4, 50.0),
        );
        self.templates.insert(
            AsciiString::from("Wedge"),
            FormationTemplate::create_wedge_formation(15, 50.0),
        );
        self.templates.insert(
            AsciiString::from("Circle"),
            FormationTemplate::create_circle_formation(12, 100.0),
        );
    }

    /// Create new formation
    pub fn create_formation(
        &mut self,
        template_name: &str,
        objects: Vec<ObjectID>,
        owner: Int,
    ) -> Option<UnsignedInt> {
        if let Some(template) = self
            .templates
            .get(&AsciiString::from(template_name))
            .cloned()
        {
            let formation_id = self.next_formation_id;
            self.next_formation_id += 1;

            let formation =
                Formation::new(formation_id, template, objects, owner, self.current_frame);
            self.formations.insert(formation_id, formation);

            // Initialize formation positions
            if let Some(formation) = self.formations.get_mut(&formation_id) {
                if let Some(lookup) = &self.object_lookup {
                    if let Ok(lookup_guard) = lookup.read() {
                        formation.assign_positions(&*lookup_guard);
                        formation.update_center(&*lookup_guard);
                    }
                }
            }

            Some(formation_id)
        } else {
            None
        }
    }

    /// Disband formation
    pub fn disband_formation(&mut self, formation_id: UnsignedInt) -> bool {
        self.formations.remove(&formation_id).is_some()
    }

    /// Update all formations
    pub fn update(&mut self, frame: UnsignedInt) {
        self.current_frame = frame;

        let mut formations_to_remove = Vec::new();

        if let Some(lookup) = &self.object_lookup {
            if let Ok(lookup_guard) = lookup.read() {
                for (formation_id, formation) in &mut self.formations {
                    formation.last_update_frame = frame;

                    // Remove dead objects
                    formation
                        .objects
                        .retain(|&obj_id| lookup_guard.is_object_alive(obj_id));

                    // Check if formation is still viable
                    if !formation.is_viable() {
                        formations_to_remove.push(*formation_id);
                        continue;
                    }

                    // Update formation state
                    formation.update_center(&*lookup_guard);
                    formation.assign_positions(&*lookup_guard);

                    // Update formation state based on conditions
                    match formation.state {
                        FormationState::Moving => {
                            if formation.has_reached_target() {
                                formation.state = FormationState::Holding;
                                formation.movement_target = None;
                            }
                        }
                        FormationState::Forming => {
                            // Check if formation has stabilized
                            formation.state = FormationState::Holding;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Remove non-viable formations
        for formation_id in formations_to_remove {
            self.formations.remove(&formation_id);
        }
    }

    /// Move formation to target
    pub fn move_formation(&mut self, formation_id: UnsignedInt, target: Coord3D) -> bool {
        if let Some(formation) = self.formations.get_mut(&formation_id) {
            formation.set_movement_target(target);
            return true;
        }
        false
    }

    /// Get formation movement orders
    pub fn get_formation_movement_orders(
        &self,
        formation_id: UnsignedInt,
    ) -> Vec<FormationMovementOrder> {
        if let Some(formation) = self.formations.get(&formation_id) {
            if let Some(lookup) = &self.object_lookup {
                if let Ok(lookup_guard) = lookup.read() {
                    return formation.get_movement_orders(&*lookup_guard);
                }
            }
        }
        Vec::new()
    }

    /// Get formation by ID
    pub fn get_formation(&self, formation_id: UnsignedInt) -> Option<&Formation> {
        self.formations.get(&formation_id)
    }

    /// Get formation by ID (mutable)
    pub fn get_formation_mut(&mut self, formation_id: UnsignedInt) -> Option<&mut Formation> {
        self.formations.get_mut(&formation_id)
    }

    /// Find formations containing object
    pub fn find_formations_with_object(&self, object_id: ObjectID) -> Vec<UnsignedInt> {
        self.formations
            .iter()
            .filter(|(_, formation)| formation.objects.contains(&object_id))
            .map(|(&id, _)| id)
            .collect()
    }

    /// Register custom formation template
    pub fn register_template(&mut self, name: AsciiString, template: FormationTemplate) {
        self.templates.insert(name, template);
    }

    /// Get available formation templates
    pub fn get_template_names(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// Get formation count
    pub fn get_formation_count(&self) -> usize {
        self.formations.len()
    }
}

impl Default for FormationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global formation manager instance
use once_cell::sync::Lazy;
static FORMATION_MANAGER: Lazy<Arc<RwLock<FormationManager>>> =
    Lazy::new(|| Arc::new(RwLock::new(FormationManager::new())));

/// Get global formation manager
pub fn get_formation_manager() -> Arc<RwLock<FormationManager>> {
    FORMATION_MANAGER.clone()
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
