//! Formation Types and Shapes
//!
//! Defines all formation types, shapes, and configuration structures.

use crate::common::{Coord3D, ObjectID, Real};
use std::f32::consts::PI;

/// Formation type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormationType {
    /// No formation - units move independently
    None,
    /// Line formation - horizontal line perpendicular to movement
    Line,
    /// Column formation - vertical column along movement direction
    Column,
    /// Wedge formation - V-shaped tactical formation
    Wedge,
    /// Box formation - rectangular grid
    Box,
    /// Scatter formation - dispersed irregular pattern
    Scatter,
    /// Custom formation from template
    Custom(u32),
}

impl Default for FormationType {
    fn default() -> Self {
        FormationType::None
    }
}

/// Scatter pattern for dispersed formations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScatterPattern {
    /// Random distribution within radius
    Random,
    /// Loose grid pattern
    LooseGrid,
    /// Irregular stagger
    Staggered,
}

/// Formation shape definition
#[derive(Debug, Clone)]
pub struct FormationShape {
    /// Formation type
    pub formation_type: FormationType,

    /// Number of units in formation
    pub unit_count: usize,

    /// Spacing between adjacent units
    pub spacing: Real,

    /// Formation width (perpendicular to movement)
    pub width: Real,

    /// Formation depth (along movement direction)
    pub depth: Real,

    /// Relative positions for each slot
    pub slots: Vec<FormationSlot>,

    /// Formation orientation (radians)
    pub orientation: Real,
}

/// Individual slot within a formation
#[derive(Debug, Clone)]
pub struct FormationSlot {
    /// Slot index
    pub index: usize,

    /// Relative position from formation center
    pub relative_position: Coord3D,

    /// Priority (0 = highest, typically leader)
    pub priority: u32,

    /// Preferred facing direction offset
    pub facing_offset: Real,

    /// Currently assigned unit
    pub assigned_unit: Option<ObjectID>,

    /// Role in formation (leader, flanker, rear guard, etc.)
    pub role: SlotRole,
}

/// Role of a slot within the formation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotRole {
    /// Formation leader
    Leader,
    /// Front line positions
    FrontLine,
    /// Flank positions
    Flanker,
    /// Center positions
    Center,
    /// Rear guard positions
    RearGuard,
    /// Support positions
    Support,
}

/// Formation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationState {
    /// Formation is being organized
    Forming,
    /// Formation is established and holding
    Formed,
    /// Formation is moving
    Moving,
    /// Formation is engaged in combat
    InCombat,
    /// Formation is breaking apart
    Breaking,
    /// Formation is attempting to reform
    Reforming,
    /// Formation is disbanded
    Disbanded,
}

/// Formation settings and behavior parameters
#[derive(Debug, Clone)]
pub struct FormationSettings {
    /// Maintain formation during movement
    pub maintain_during_movement: bool,

    /// Maintain formation during combat
    pub maintain_during_combat: bool,

    /// Formation stiffness (0.0 = loose, 1.0 = rigid)
    pub stiffness: Real,

    /// Maximum deviation from assigned position
    pub max_deviation: Real,

    /// Speed at which to reform
    pub reformation_speed: Real,

    /// Distance at which formation breaks
    pub break_distance: Real,

    /// Time before attempting reform (seconds)
    pub reform_delay: Real,

    /// Automatically reform after combat
    pub auto_reform_after_combat: bool,

    /// Allow rotation to face movement direction
    pub rotate_to_movement: bool,

    /// Prefer tighter spacing in combat
    pub compress_in_combat: bool,

    /// Formation elasticity (ability to stretch/compress)
    pub elasticity: Real,
}

impl Default for FormationSettings {
    fn default() -> Self {
        Self {
            maintain_during_movement: true,
            maintain_during_combat: false,
            stiffness: 0.7,
            max_deviation: 75.0,
            reformation_speed: 0.15,
            break_distance: 500.0,
            reform_delay: 2.0,
            auto_reform_after_combat: true,
            rotate_to_movement: true,
            compress_in_combat: true,
            elasticity: 0.3,
        }
    }
}

impl FormationShape {
    /// Create a line formation
    pub fn create_line(unit_count: usize, spacing: Real) -> Self {
        let mut slots = Vec::with_capacity(unit_count);
        let half_width = (unit_count as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let x_offset = i as Real * spacing - half_width;
            slots.push(FormationSlot {
                index: i,
                relative_position: Coord3D::new(x_offset, 0.0, 0.0),
                priority: if i == unit_count / 2 { 0 } else { i as u32 + 1 },
                facing_offset: 0.0,
                assigned_unit: None,
                role: if i == unit_count / 2 {
                    SlotRole::Leader
                } else if i == 0 || i == unit_count - 1 {
                    SlotRole::Flanker
                } else {
                    SlotRole::FrontLine
                },
            });
        }

        Self {
            formation_type: FormationType::Line,
            unit_count,
            spacing,
            width: (unit_count as Real - 1.0) * spacing,
            depth: 0.0,
            slots,
            orientation: 0.0,
        }
    }

    /// Create a column formation
    pub fn create_column(unit_count: usize, spacing: Real) -> Self {
        let mut slots = Vec::with_capacity(unit_count);
        let half_depth = (unit_count as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let y_offset = i as Real * spacing - half_depth;
            slots.push(FormationSlot {
                index: i,
                relative_position: Coord3D::new(0.0, y_offset, 0.0),
                priority: i as u32,
                facing_offset: 0.0,
                assigned_unit: None,
                role: if i == 0 {
                    SlotRole::Leader
                } else if i == unit_count - 1 {
                    SlotRole::RearGuard
                } else {
                    SlotRole::Center
                },
            });
        }

        Self {
            formation_type: FormationType::Column,
            unit_count,
            spacing,
            width: 0.0,
            depth: (unit_count as Real - 1.0) * spacing,
            slots,
            orientation: 0.0,
        }
    }

    /// Create a wedge formation
    pub fn create_wedge(unit_count: usize, spacing: Real) -> Self {
        let mut slots = Vec::with_capacity(unit_count);

        // Leader at the point
        slots.push(FormationSlot {
            index: 0,
            relative_position: Coord3D::new(0.0, 0.0, 0.0),
            priority: 0,
            facing_offset: 0.0,
            assigned_unit: None,
            role: SlotRole::Leader,
        });

        let mut units_placed = 1;
        let mut row = 1;

        // Build the wedge backwards from the point
        while units_placed < unit_count {
            let units_in_row = (row * 2).min(unit_count - units_placed);
            let row_y = -(row as Real) * spacing * 0.866; // sqrt(3)/2 for equilateral

            for i in 0..units_in_row {
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let x_offset = side * ((i / 2 + 1) as Real) * spacing;

                slots.push(FormationSlot {
                    index: units_placed,
                    relative_position: Coord3D::new(x_offset, row_y, 0.0),
                    priority: row as u32,
                    facing_offset: 0.0,
                    assigned_unit: None,
                    role: if i == 0 || i == units_in_row - 1 {
                        SlotRole::Flanker
                    } else {
                        SlotRole::FrontLine
                    },
                });

                units_placed += 1;
                if units_placed >= unit_count {
                    break;
                }
            }

            row += 1;
        }

        let max_width = spacing * ((row - 1) * 2) as Real;
        let max_depth = spacing * (row - 1) as Real * 0.866;

        Self {
            formation_type: FormationType::Wedge,
            unit_count,
            spacing,
            width: max_width,
            depth: max_depth,
            slots,
            orientation: 0.0,
        }
    }

    /// Create a box formation
    pub fn create_box(unit_count: usize, spacing: Real) -> Self {
        let side_length = (unit_count as f32).sqrt().ceil() as usize;
        let mut slots = Vec::with_capacity(unit_count);

        let half_width = (side_length as Real - 1.0) * spacing * 0.5;
        let half_depth = (side_length as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let row = i / side_length;
            let col = i % side_length;

            let x_offset = col as Real * spacing - half_width;
            let y_offset = row as Real * spacing - half_depth;

            let role = if row == 0 {
                if col == 0 || col == side_length - 1 {
                    SlotRole::Flanker
                } else {
                    SlotRole::FrontLine
                }
            } else if row == side_length - 1 {
                SlotRole::RearGuard
            } else {
                SlotRole::Center
            };

            slots.push(FormationSlot {
                index: i,
                relative_position: Coord3D::new(x_offset, y_offset, 0.0),
                priority: if i == 0 {
                    0
                } else {
                    (row * side_length + col) as u32
                },
                facing_offset: 0.0,
                assigned_unit: None,
                role,
            });
        }

        Self {
            formation_type: FormationType::Box,
            unit_count,
            spacing,
            width: (side_length as Real - 1.0) * spacing,
            depth: (side_length as Real - 1.0) * spacing,
            slots,
            orientation: 0.0,
        }
    }

    /// Create a scatter formation
    pub fn create_scatter(unit_count: usize, spacing: Real, pattern: ScatterPattern) -> Self {
        let mut slots = Vec::with_capacity(unit_count);
        let radius = spacing * (unit_count as Real / 2.0).sqrt();

        match pattern {
            ScatterPattern::Random => {
                // Use deterministic pseudo-random for consistency
                for i in 0..unit_count {
                    let angle = (i as Real * 2.39996323) % (2.0 * PI); // Golden angle
                    let r = radius * ((i as Real / unit_count as Real).sqrt());
                    let x = r * angle.cos();
                    let y = r * angle.sin();

                    slots.push(FormationSlot {
                        index: i,
                        relative_position: Coord3D::new(x, y, 0.0),
                        priority: i as u32,
                        facing_offset: 0.0,
                        assigned_unit: None,
                        role: if i == 0 {
                            SlotRole::Leader
                        } else {
                            SlotRole::Support
                        },
                    });
                }
            }
            ScatterPattern::LooseGrid => {
                let grid_spacing = spacing * 1.5;
                let side_length = (unit_count as f32).sqrt().ceil() as usize;

                for i in 0..unit_count {
                    let row = i / side_length;
                    let col = i % side_length;
                    let x = (col as Real - side_length as Real * 0.5) * grid_spacing;
                    let y = (row as Real - side_length as Real * 0.5) * grid_spacing;

                    slots.push(FormationSlot {
                        index: i,
                        relative_position: Coord3D::new(x, y, 0.0),
                        priority: i as u32,
                        facing_offset: 0.0,
                        assigned_unit: None,
                        role: if i == 0 {
                            SlotRole::Leader
                        } else {
                            SlotRole::Support
                        },
                    });
                }
            }
            ScatterPattern::Staggered => {
                let rows = ((unit_count as f32).sqrt().ceil()) as usize;
                let mut units_placed = 0;

                for row in 0..rows {
                    let units_in_row =
                        (unit_count - units_placed).min(if row % 2 == 0 { rows } else { rows - 1 });

                    for col in 0..units_in_row {
                        let x_offset = if row % 2 == 0 {
                            col as Real * spacing
                        } else {
                            (col as Real + 0.5) * spacing
                        };
                        let y = row as Real * spacing * 0.866;
                        let x = x_offset - (units_in_row as Real * spacing * 0.5);

                        slots.push(FormationSlot {
                            index: units_placed,
                            relative_position: Coord3D::new(x, y, 0.0),
                            priority: units_placed as u32,
                            facing_offset: 0.0,
                            assigned_unit: None,
                            role: if units_placed == 0 {
                                SlotRole::Leader
                            } else {
                                SlotRole::Support
                            },
                        });

                        units_placed += 1;
                        if units_placed >= unit_count {
                            break;
                        }
                    }

                    if units_placed >= unit_count {
                        break;
                    }
                }
            }
        }

        Self {
            formation_type: FormationType::Scatter,
            unit_count,
            spacing,
            width: radius * 2.0,
            depth: radius * 2.0,
            slots,
            orientation: 0.0,
        }
    }

    /// Rotate formation to new orientation
    pub fn rotate(&mut self, new_orientation: Real) {
        let delta = new_orientation - self.orientation;
        let cos_theta = delta.cos();
        let sin_theta = delta.sin();

        for slot in &mut self.slots {
            let x = slot.relative_position.x;
            let y = slot.relative_position.y;

            slot.relative_position.x = x * cos_theta - y * sin_theta;
            slot.relative_position.y = x * sin_theta + y * cos_theta;
        }

        self.orientation = new_orientation;
    }

    /// Scale formation by factor
    pub fn scale(&mut self, factor: Real) {
        for slot in &mut self.slots {
            slot.relative_position.x *= factor;
            slot.relative_position.y *= factor;
        }

        self.spacing *= factor;
        self.width *= factor;
        self.depth *= factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_formation() {
        let formation = FormationShape::create_line(5, 50.0);
        assert_eq!(formation.unit_count, 5);
        assert_eq!(formation.slots.len(), 5);
        assert_eq!(formation.formation_type, FormationType::Line);

        // Check leader is in center
        assert_eq!(formation.slots[2].role, SlotRole::Leader);
        assert_eq!(formation.slots[2].priority, 0);
    }

    #[test]
    fn test_column_formation() {
        let formation = FormationShape::create_column(4, 40.0);
        assert_eq!(formation.unit_count, 4);
        assert_eq!(formation.formation_type, FormationType::Column);

        // Check leader is at front
        assert_eq!(formation.slots[0].role, SlotRole::Leader);

        // Check rear guard is at back
        assert_eq!(formation.slots[3].role, SlotRole::RearGuard);
    }

    #[test]
    fn test_wedge_formation() {
        let formation = FormationShape::create_wedge(7, 50.0);
        assert_eq!(formation.unit_count, 7);
        assert_eq!(formation.formation_type, FormationType::Wedge);

        // Leader at point
        assert_eq!(formation.slots[0].role, SlotRole::Leader);
        assert_eq!(formation.slots[0].relative_position.x, 0.0);
        assert_eq!(formation.slots[0].relative_position.y, 0.0);
    }

    #[test]
    fn test_box_formation() {
        let formation = FormationShape::create_box(9, 50.0);
        assert_eq!(formation.unit_count, 9);
        assert_eq!(formation.formation_type, FormationType::Box);
    }

    #[test]
    fn test_scatter_formation() {
        let formation = FormationShape::create_scatter(10, 50.0, ScatterPattern::Random);
        assert_eq!(formation.unit_count, 10);
        assert_eq!(formation.formation_type, FormationType::Scatter);
    }

    #[test]
    fn test_formation_rotation() {
        let mut formation = FormationShape::create_line(3, 50.0);
        let original_x = formation.slots[0].relative_position.x;

        formation.rotate(PI / 2.0); // 90 degrees

        // After 90 degree rotation, x becomes -y
        let expected_y = original_x;
        let actual_y = formation.slots[0].relative_position.y;

        assert!((actual_y - expected_y).abs() < 0.1);
    }

    #[test]
    fn test_formation_scaling() {
        let mut formation = FormationShape::create_line(3, 50.0);
        let original_spacing = formation.spacing;

        formation.scale(2.0);

        assert_eq!(formation.spacing, original_spacing * 2.0);
    }
}
