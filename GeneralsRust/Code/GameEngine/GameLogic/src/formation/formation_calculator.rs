//! Formation Calculator
//!
//! Calculates formation positions, layouts, and transformations.

use super::formation_types::{FormationShape, FormationSlot, FormationType, ScatterPattern};
use super::{FormationError, FormationResult, DEFAULT_SPACING};
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::HashMap;

/// Formation layout with world positions
#[derive(Debug, Clone)]
pub struct FormationLayout {
    /// Formation center in world coordinates
    pub center: Coord3D,

    /// Formation heading (radians)
    pub heading: Real,

    /// Assigned positions for each unit
    pub positions: HashMap<ObjectID, Coord3D>,

    /// Formation shape
    pub shape: FormationShape,
}

/// Position calculator for formations
pub struct PositionCalculator {
    /// Current formation shape
    shape: FormationShape,

    /// World center position
    center: Coord3D,

    /// Formation heading
    heading: Real,
}

impl PositionCalculator {
    /// Create new position calculator
    pub fn new(shape: FormationShape, center: Coord3D, heading: Real) -> Self {
        Self {
            shape,
            center,
            heading,
        }
    }

    /// Calculate world position for a slot
    pub fn calculate_world_position(&self, slot: &FormationSlot) -> Coord3D {
        self.transform_to_world(&slot.relative_position)
    }

    /// Transform relative position to world coordinates
    fn transform_to_world(&self, relative_pos: &Coord3D) -> Coord3D {
        let cos_heading = self.heading.cos();
        let sin_heading = self.heading.sin();

        // Rotate by heading
        let rotated_x = relative_pos.x * cos_heading - relative_pos.y * sin_heading;
        let rotated_y = relative_pos.x * sin_heading + relative_pos.y * cos_heading;

        // Translate to world center
        Coord3D::new(
            self.center.x + rotated_x,
            self.center.y + rotated_y,
            self.center.z + relative_pos.z,
        )
    }

    /// Transform world position to relative coordinates
    fn transform_to_relative(&self, world_pos: &Coord3D) -> Coord3D {
        // Translate to formation center
        let local_x = world_pos.x - self.center.x;
        let local_y = world_pos.y - self.center.y;
        let local_z = world_pos.z - self.center.z;

        // Rotate by negative heading
        let cos_heading = (-self.heading).cos();
        let sin_heading = (-self.heading).sin();

        Coord3D::new(
            local_x * cos_heading - local_y * sin_heading,
            local_x * sin_heading + local_y * cos_heading,
            local_z,
        )
    }

    /// Find nearest available slot for a position
    pub fn find_nearest_slot(&self, position: &Coord3D) -> Option<usize> {
        let relative_pos = self.transform_to_relative(position);

        let mut nearest_slot = None;
        let mut min_distance = Real::INFINITY;

        for slot in &self.shape.slots {
            if slot.assigned_unit.is_none() {
                let distance = Self::distance_3d(&slot.relative_position, &relative_pos);
                if distance < min_distance {
                    min_distance = distance;
                    nearest_slot = Some(slot.index);
                }
            }
        }

        nearest_slot
    }

    /// Calculate optimal slot assignment for units
    pub fn assign_slots_optimal(
        &self,
        unit_positions: &HashMap<ObjectID, Coord3D>,
    ) -> HashMap<ObjectID, usize> {
        let mut assignments = HashMap::new();
        let mut used_slots = std::collections::HashSet::new();

        // Convert unit positions to relative coordinates
        let relative_positions: HashMap<ObjectID, Coord3D> = unit_positions
            .iter()
            .map(|(&id, pos)| (id, self.transform_to_relative(pos)))
            .collect();

        // Sort units by priority (closest to formation center first)
        let mut units: Vec<_> = relative_positions.iter().collect();
        units.sort_by(|a, b| {
            let dist_a = Self::distance_3d(a.1, &Coord3D::new(0.0, 0.0, 0.0));
            let dist_b = Self::distance_3d(b.1, &Coord3D::new(0.0, 0.0, 0.0));
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Assign each unit to nearest available slot
        for (&unit_id, unit_pos) in units {
            let mut best_slot = None;
            let mut best_distance = Real::INFINITY;

            for slot in &self.shape.slots {
                if !used_slots.contains(&slot.index) {
                    let distance = Self::distance_3d(&slot.relative_position, unit_pos);
                    if distance < best_distance {
                        best_distance = distance;
                        best_slot = Some(slot.index);
                    }
                }
            }

            if let Some(slot_index) = best_slot {
                assignments.insert(unit_id, slot_index);
                used_slots.insert(slot_index);
            }
        }

        assignments
    }

    /// Update formation center
    pub fn set_center(&mut self, center: Coord3D) {
        self.center = center;
    }

    /// Update formation heading
    pub fn set_heading(&mut self, heading: Real) {
        self.heading = heading;
    }

    /// Calculate distance between two 3D points
    fn distance_3d(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Formation calculator - main interface for formation calculations
pub struct FormationCalculator {
    /// Cached formation shapes
    shape_cache: HashMap<(FormationType, usize), FormationShape>,

    /// Default spacing
    default_spacing: Real,
}

impl FormationCalculator {
    /// Create new formation calculator
    pub fn new() -> Self {
        Self {
            shape_cache: HashMap::new(),
            default_spacing: DEFAULT_SPACING,
        }
    }

    /// Get or create formation shape
    pub fn get_shape(
        &mut self,
        formation_type: FormationType,
        unit_count: usize,
        spacing: Option<Real>,
    ) -> FormationShape {
        let spacing = spacing.unwrap_or(self.default_spacing);
        let key = (formation_type, unit_count);

        if let Some(shape) = self.shape_cache.get(&key) {
            return shape.clone();
        }

        let shape = match formation_type {
            FormationType::None => {
                // No formation - units stay where they are
                FormationShape::create_scatter(unit_count, spacing, ScatterPattern::Random)
            }
            FormationType::Line => FormationShape::create_line(unit_count, spacing),
            FormationType::Column => FormationShape::create_column(unit_count, spacing),
            FormationType::Wedge => FormationShape::create_wedge(unit_count, spacing),
            FormationType::Box => FormationShape::create_box(unit_count, spacing),
            FormationType::Scatter => {
                FormationShape::create_scatter(unit_count, spacing, ScatterPattern::Random)
            }
            FormationType::Custom(_) => {
                // Custom formations would load from templates
                FormationShape::create_line(unit_count, spacing)
            }
        };

        self.shape_cache.insert(key, shape.clone());
        shape
    }

    /// Create formation layout
    pub fn create_layout(
        &mut self,
        formation_type: FormationType,
        unit_positions: &HashMap<ObjectID, Coord3D>,
        heading: Real,
        spacing: Option<Real>,
    ) -> FormationResult<FormationLayout> {
        if unit_positions.is_empty() {
            return Err(FormationError::NoUnits);
        }

        let unit_count = unit_positions.len();
        let shape = self.get_shape(formation_type, unit_count, spacing);

        // Calculate center of mass
        let center = self.calculate_center_of_mass(unit_positions);

        // Create position calculator
        let calculator = PositionCalculator::new(shape.clone(), center, heading);

        // Assign slots
        let assignments = calculator.assign_slots_optimal(unit_positions);

        // Calculate world positions for each unit
        let mut positions = HashMap::new();
        for (&unit_id, &slot_index) in &assignments {
            if let Some(slot) = shape.slots.get(slot_index) {
                let world_pos = calculator.calculate_world_position(slot);
                positions.insert(unit_id, world_pos);
            }
        }

        Ok(FormationLayout {
            center,
            heading,
            positions,
            shape,
        })
    }

    /// Calculate center of mass for units
    fn calculate_center_of_mass(&self, unit_positions: &HashMap<ObjectID, Coord3D>) -> Coord3D {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;
        let count = unit_positions.len() as Real;

        for pos in unit_positions.values() {
            sum_x += pos.x;
            sum_y += pos.y;
            sum_z += pos.z;
        }

        Coord3D::new(sum_x / count, sum_y / count, sum_z / count)
    }

    /// Calculate heading from current positions toward target
    pub fn calculate_heading_to_target(current_center: &Coord3D, target: &Coord3D) -> Real {
        let dx = target.x - current_center.x;
        let dy = target.y - current_center.y;
        dy.atan2(dx)
    }

    /// Update layout for movement
    pub fn update_layout_for_movement(
        &mut self,
        layout: &mut FormationLayout,
        new_center: Coord3D,
        new_heading: Real,
    ) {
        layout.center = new_center;
        layout.heading = new_heading;

        // Recalculate positions
        let calculator = PositionCalculator::new(layout.shape.clone(), new_center, new_heading);

        for (unit_id, slot) in layout
            .shape
            .slots
            .iter()
            .filter_map(|s| s.assigned_unit.map(|id| (id, s)))
        {
            let world_pos = calculator.calculate_world_position(slot);
            layout.positions.insert(unit_id, world_pos);
        }
    }

    /// Scale formation by factor
    pub fn scale_formation(&mut self, layout: &mut FormationLayout, scale_factor: Real) {
        layout.shape.scale(scale_factor);

        // Recalculate positions
        let calculator =
            PositionCalculator::new(layout.shape.clone(), layout.center, layout.heading);

        for (unit_id, slot) in layout
            .shape
            .slots
            .iter()
            .filter_map(|s| s.assigned_unit.map(|id| (id, s)))
        {
            let world_pos = calculator.calculate_world_position(slot);
            layout.positions.insert(unit_id, world_pos);
        }
    }

    /// Check if positions match formation (within tolerance)
    pub fn check_formation_coherence(
        layout: &FormationLayout,
        current_positions: &HashMap<ObjectID, Coord3D>,
        tolerance: Real,
    ) -> Real {
        if layout.positions.is_empty() {
            return 1.0;
        }

        let mut in_position_count = 0;
        let mut total_count = 0;

        for (&unit_id, target_pos) in &layout.positions {
            if let Some(current_pos) = current_positions.get(&unit_id) {
                let distance = Self::distance_3d(current_pos, target_pos);
                if distance < tolerance {
                    in_position_count += 1;
                }
                total_count += 1;
            }
        }

        if total_count > 0 {
            in_position_count as Real / total_count as Real
        } else {
            0.0
        }
    }

    /// Calculate distance between two points
    fn distance_3d(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Set default spacing
    pub fn set_default_spacing(&mut self, spacing: Real) {
        self.default_spacing = spacing;
        self.shape_cache.clear(); // Clear cache when spacing changes
    }

    /// Clear shape cache
    pub fn clear_cache(&mut self) {
        self.shape_cache.clear();
    }
}

impl Default for FormationCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_calculator() {
        let shape = FormationShape::create_line(3, 50.0);
        let center = Coord3D::new(100.0, 100.0, 0.0);
        let calculator = PositionCalculator::new(shape.clone(), center, 0.0);

        let world_pos = calculator.calculate_world_position(&shape.slots[0]);
        assert!(world_pos.x != shape.slots[0].relative_position.x); // Should be transformed
    }

    #[test]
    fn test_formation_calculator() {
        let mut calculator = FormationCalculator::new();

        let mut positions = HashMap::new();
        positions.insert(100, Coord3D::new(0.0, 0.0, 0.0));
        positions.insert(101, Coord3D::new(10.0, 0.0, 0.0));
        positions.insert(102, Coord3D::new(20.0, 0.0, 0.0));

        let layout = calculator
            .create_layout(FormationType::Line, &positions, 0.0, None)
            .unwrap();

        assert_eq!(layout.positions.len(), 3);
    }

    #[test]
    fn test_heading_calculation() {
        let current = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(10.0, 10.0, 0.0);

        let heading = FormationCalculator::calculate_heading_to_target(&current, &target);
        assert!(heading > 0.0); // Should point northeast
    }

    #[test]
    fn test_formation_coherence() {
        let mut calculator = FormationCalculator::new();

        let mut positions = HashMap::new();
        positions.insert(100, Coord3D::new(0.0, 0.0, 0.0));
        positions.insert(101, Coord3D::new(10.0, 0.0, 0.0));

        let layout = calculator
            .create_layout(FormationType::Line, &positions, 0.0, None)
            .unwrap();

        // All units at their target positions
        let coherence =
            FormationCalculator::check_formation_coherence(&layout, &layout.positions, 10.0);

        assert_eq!(coherence, 1.0);
    }
}
