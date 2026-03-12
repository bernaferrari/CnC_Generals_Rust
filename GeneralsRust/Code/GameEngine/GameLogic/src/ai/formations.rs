//! Formation system for group unit movement
//!
//! This module implements formation offset calculations for group movement,
//! matching the C++ implementation in AIGroup.cpp.
//!
//! Author: Converted from C++ original by Michael S. Booth

use crate::common::{Coord2D, Coord3D, Real};
use std::f32::consts::PI;

/// Formation types for unit groups
///
/// Matches C++ AIGroup formation behavior from /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationType {
    /// Wedge formation - V-shaped arrangement with leader at front
    Wedge,
    /// Column formation - units arranged in vertical columns
    Column,
    /// Cluster formation - loose blob arrangement around center
    Cluster,
}

/// Formation configuration parameters
#[derive(Debug, Clone)]
pub struct FormationConfig {
    /// Base angle for wedge formation (radians)
    pub wedge_base_angle: Real,
    /// Angle step between units in wedge
    pub wedge_angle_step: Real,
    /// Base distance from leader
    pub base_distance: Real,
    /// Spacing between rows
    pub row_spacing: Real,
    /// Number of columns for column formation
    pub num_columns: i32,
    /// Spread threshold for tightening (game units)
    pub spread_threshold: Real,
}

impl Default for FormationConfig {
    fn default() -> Self {
        // Matches C++ AIGroup.cpp default formation parameters
        Self {
            wedge_base_angle: PI / 6.0,  // 30 degrees
            wedge_angle_step: PI / 12.0, // 15 degrees
            base_distance: 10.0,         // Base distance from leader
            row_spacing: 8.0,            // Space between rows
            num_columns: 3,              // Default 3 columns for infantry
            spread_threshold: 50.0,      // Threshold to trigger tightening
        }
    }
}

/// Calculate formation offset for a unit based on its index
///
/// This function computes the position offset for a unit in a formation
/// relative to the formation leader.
///
/// # Arguments
///
/// * `formation_type` - Type of formation (Wedge, Column, or Cluster)
/// * `unit_index` - Index of unit in formation (0 = leader)
/// * `total_units` - Total number of units in formation
/// * `leader_facing` - Direction the leader is facing (radians)
/// * `config` - Formation configuration parameters
///
/// # Returns
///
/// Returns a 2D offset vector from the leader position
///
/// # Examples
///
/// ```
/// use gamelogic::ai::formations::{FormationConfig, FormationType, calculate_formation_offset};
/// use std::f32::consts::PI;
///
/// let config = FormationConfig::default();
/// let offset = calculate_formation_offset(
///     FormationType::Wedge,
///     1,
///     5,
///     0.0,
///     &config
/// );
/// // Unit 1 will be offset to the left of leader
/// ```
pub fn calculate_formation_offset(
    formation_type: FormationType,
    unit_index: usize,
    total_units: usize,
    leader_facing: Real,
    config: &FormationConfig,
) -> Coord2D {
    // Leader is always at origin (no offset)
    if unit_index == 0 {
        return Coord2D::new(0.0, 0.0);
    }

    match formation_type {
        FormationType::Wedge => {
            calculate_wedge_offset(unit_index, total_units, leader_facing, config)
        }
        FormationType::Column => {
            calculate_column_offset(unit_index, total_units, leader_facing, config)
        }
        FormationType::Cluster => {
            calculate_cluster_offset(unit_index, total_units, leader_facing, config)
        }
    }
}

/// Calculate wedge formation offset
///
/// Creates a V-shaped formation with units alternating left/right
/// behind the leader.
///
/// Algorithm from C++ AIGroup.cpp lines 800-975
fn calculate_wedge_offset(
    unit_index: usize,
    _total_units: usize,
    leader_facing: Real,
    config: &FormationConfig,
) -> Coord2D {
    // Calculate which row this unit is in
    let row = (unit_index - 1) / 2;

    // Alternate left (-1) and right (+1)
    let side = if (unit_index - 1) % 2 == 0 { -1.0 } else { 1.0 };

    // Followers form a V *behind* the leader (opposite of the leader's facing).
    // For leader_facing == 0, followers should be at negative X (behind), with
    // left/right determined by +/- Y.
    let angle_offset = config.wedge_base_angle + (row as Real * config.wedge_angle_step);
    let angle = PI + (angle_offset * side) - leader_facing;

    // Calculate distance from leader
    // Units further back are also further away
    let distance = config.base_distance + (row as Real * config.row_spacing);

    // Apply rotation matrix for leader's facing direction
    Coord2D::new(angle.cos() * distance, angle.sin() * distance)
}

/// Calculate column formation offset
///
/// Creates columns of units marching in formation.
/// Matches C++ AIGroup.cpp friend_moveInfantryToPos logic (lines 653-975)
fn calculate_column_offset(
    unit_index: usize,
    total_units: usize,
    leader_facing: Real,
    config: &FormationConfig,
) -> Coord2D {
    let num_columns = config.num_columns;
    let half_num_columns = num_columns / 2;

    // Calculate row and column for this unit
    let row = (unit_index - 1) / num_columns as usize;
    let col = (unit_index - 1) % num_columns as usize;

    // Column delta: -half_columns to +half_columns
    // For 3 columns: -1, 0, 1 (left, center, right)
    let column_delta = (col as i32) - half_num_columns;

    // Distance behind leader (rows stack backwards)
    let forward_offset = -(row as Real * config.row_spacing);

    // Distance left/right from center
    let lateral_offset = (column_delta as Real) * config.base_distance;

    // Rotate local offsets by `-leader_facing` (clockwise-positive facing).
    let cos_facing = leader_facing.cos();
    let sin_facing = leader_facing.sin();

    Coord2D::new(
        forward_offset * cos_facing + lateral_offset * sin_facing,
        -forward_offset * sin_facing + lateral_offset * cos_facing,
    )
}

/// Calculate cluster formation offset
///
/// Creates a loose blob formation around the leader.
/// Units are arranged in a circular pattern.
fn calculate_cluster_offset(
    unit_index: usize,
    total_units: usize,
    _leader_facing: Real,
    config: &FormationConfig,
) -> Coord2D {
    let _ = total_units;

    // Unit index 0 is the leader at origin. Followers (1..) are distributed in
    // concentric rings around the leader:
    // ring 1: 6 units, ring 2: 12 units, ring 3: 18 units, ...
    let follower_index = unit_index.saturating_sub(1);

    let mut units_placed: usize = 0;
    let mut ring: usize = 1;
    while follower_index >= units_placed + ring_capacity(ring) {
        units_placed += ring_capacity(ring);
        ring += 1;
    }

    let position_in_ring = follower_index - units_placed;
    let units_in_ring = ring_capacity(ring);
    let angle = (position_in_ring as Real / units_in_ring as Real) * 2.0 * PI;
    let distance = ring as Real * config.base_distance;

    Coord2D::new(angle.cos() * distance, angle.sin() * distance)
}

/// Helper function to calculate how many units fit in a ring
fn ring_capacity(ring: usize) -> usize {
    if ring == 0 {
        1 // Leader
    } else {
        6 * ring // Hexagonal packing
    }
}

/// Calculate the center of mass for a group of units
///
/// # Arguments
///
/// * `positions` - Slice of unit positions
///
/// # Returns
///
/// Returns the average position of all units
pub fn calculate_group_center(positions: &[Coord3D]) -> Coord3D {
    if positions.is_empty() {
        return Coord3D::new(0.0, 0.0, 0.0);
    }

    let sum = positions
        .iter()
        .fold(Coord3D::new(0.0, 0.0, 0.0), |acc, pos| {
            Coord3D::new(acc.x + pos.x, acc.y + pos.y, acc.z + pos.z)
        });

    let count = positions.len() as Real;
    Coord3D::new(sum.x / count, sum.y / count, sum.z / count)
}

/// Calculate group spread distance
///
/// Computes the maximum distance from the group center to any unit.
/// This is used to determine if the group is too spread out and needs
/// to tighten formation.
///
/// # Arguments
///
/// * `positions` - Slice of unit positions
///
/// # Returns
///
/// Returns the maximum distance from center to any unit
///
/// # Examples
///
/// ```
/// use gamelogic::ai::formations::calculate_group_spread;
/// use gamelogic::common::Coord3D;
///
/// let positions = vec![
///     Coord3D::new(0.0, 0.0, 0.0),
///     Coord3D::new(10.0, 0.0, 0.0),
///     Coord3D::new(0.0, 10.0, 0.0),
/// ];
/// let spread = calculate_group_spread(&positions);
/// // spread will be approximately 6.67 (distance from center to furthest unit)
/// ```
pub fn calculate_group_spread(positions: &[Coord3D]) -> Real {
    if positions.is_empty() {
        return 0.0;
    }

    let center = calculate_group_center(positions);

    // Find maximum distance from center
    let mut max_distance = 0.0;

    for pos in positions {
        let dx = pos.x - center.x;
        let dy = pos.y - center.y;
        let distance_sq = dx * dx + dy * dy;

        if distance_sq > max_distance * max_distance {
            max_distance = distance_sq.sqrt();
        }
    }

    max_distance
}

/// Check if group is too spread out
///
/// Determines if a group needs to tighten its formation based on
/// the spread threshold.
///
/// # Arguments
///
/// * `positions` - Slice of unit positions
/// * `threshold` - Maximum allowed spread distance
///
/// # Returns
///
/// Returns true if group spread exceeds threshold
///
/// # Examples
///
/// ```
/// use gamelogic::ai::formations::is_group_too_spread;
/// use gamelogic::common::Coord3D;
///
/// let positions = vec![
///     Coord3D::new(0.0, 0.0, 0.0),
///     Coord3D::new(100.0, 0.0, 0.0),
/// ];
/// assert!(is_group_too_spread(&positions, 30.0));
/// ```
pub fn is_group_too_spread(positions: &[Coord3D], threshold: Real) -> bool {
    calculate_group_spread(positions) > threshold
}

/// Calculate formation offset with rotation
///
/// Calculates the offset and applies rotation to align with group direction.
/// This is used when the group is moving along a path.
///
/// # Arguments
///
/// * `formation_type` - Type of formation
/// * `unit_index` - Index of unit in formation
/// * `total_units` - Total number of units
/// * `group_direction` - Direction vector of group movement
/// * `config` - Formation configuration
///
/// # Returns
///
/// Returns 3D offset position (z is always 0)
pub fn calculate_formation_offset_with_direction(
    formation_type: FormationType,
    unit_index: usize,
    total_units: usize,
    group_direction: Coord2D,
    config: &FormationConfig,
) -> Coord3D {
    // The legacy coordinate system uses clockwise-positive facing angles.
    // `atan2` returns a CCW-positive angle, so negate it.
    let leader_facing = (-group_direction.y).atan2(group_direction.x);

    let offset_2d = calculate_formation_offset(
        formation_type,
        unit_index,
        total_units,
        leader_facing,
        config,
    );

    Coord3D::new(offset_2d.x, offset_2d.y, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_wedge_formation_leader() {
        let config = FormationConfig::default();
        let offset = calculate_formation_offset(FormationType::Wedge, 0, 5, 0.0, &config);

        // Leader should have no offset
        assert_eq!(offset.x, 0.0);
        assert_eq!(offset.y, 0.0);
    }

    #[test]
    fn test_wedge_formation_alternating_sides() {
        let config = FormationConfig::default();

        // First follower (index 1) should be on left
        let offset1 = calculate_formation_offset(FormationType::Wedge, 1, 5, 0.0, &config);

        // Second follower (index 2) should be on right
        let offset2 = calculate_formation_offset(FormationType::Wedge, 2, 5, 0.0, &config);

        // They should be on opposite sides (y coordinates have opposite signs)
        assert!(offset1.y * offset2.y < 0.0);

        // Both should be behind leader (negative x for 0 facing)
        assert!(offset1.x <= 0.0);
        assert!(offset2.x <= 0.0);
    }

    #[test]
    fn test_column_formation_three_columns() {
        let config = FormationConfig {
            num_columns: 3,
            ..Default::default()
        };

        // First row: indices 1, 2, 3 should be in left, center, right columns
        let offset1 = calculate_column_offset(1, 10, 0.0, &config);
        let offset2 = calculate_column_offset(2, 10, 0.0, &config);
        let offset3 = calculate_column_offset(3, 10, 0.0, &config);

        // All should be in same row (same x coordinate, approximately)
        assert!((offset1.x - offset2.x).abs() < 0.1);
        assert!((offset2.x - offset3.x).abs() < 0.1);

        // Should be ordered left to right
        assert!(offset1.y < offset2.y);
        assert!(offset2.y < offset3.y);
    }

    #[test]
    fn test_column_formation_rows() {
        let config = FormationConfig {
            num_columns: 3,
            row_spacing: 10.0,
            ..Default::default()
        };

        // Units in different rows should have different forward offsets
        let offset_row1 = calculate_column_offset(1, 10, 0.0, &config);
        let offset_row2 = calculate_column_offset(4, 10, 0.0, &config);

        // Second row should be further back
        assert!(offset_row2.x < offset_row1.x);

        // Difference should be approximately row_spacing
        let row_diff = offset_row1.x - offset_row2.x;
        assert!((row_diff - config.row_spacing).abs() < 0.1);
    }

    #[test]
    fn test_cluster_formation_circular() {
        let config = FormationConfig::default();

        // Units in same ring should be roughly equidistant from center
        let offset1 = calculate_cluster_offset(1, 10, 0.0, &config);
        let offset2 = calculate_cluster_offset(2, 10, 0.0, &config);

        let dist1 = (offset1.x * offset1.x + offset1.y * offset1.y).sqrt();
        let dist2 = (offset2.x * offset2.x + offset2.y * offset2.y).sqrt();

        // Both in first ring, should have same distance
        assert!((dist1 - dist2).abs() < 0.1);
    }

    #[test]
    fn test_group_center_calculation() {
        let positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
        ];

        let center = calculate_group_center(&positions);

        // Center should be at average position
        assert!((center.x - 10.0 / 3.0).abs() < 0.01);
        assert!((center.y - 10.0 / 3.0).abs() < 0.01);
        assert_eq!(center.z, 0.0);
    }

    #[test]
    fn test_group_spread_calculation() {
        let positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
        ];

        let spread = calculate_group_spread(&positions);

        // Spread should be distance from center to furthest unit
        // Center is at (10/3, 10/3), furthest point is roughly 6.67 units away
        assert!(spread > 6.0 && spread < 8.0);
    }

    #[test]
    fn test_is_group_too_spread() {
        let positions_tight = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(5.0, 0.0, 0.0),
            Coord3D::new(0.0, 5.0, 0.0),
        ];

        let positions_spread = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(100.0, 0.0, 0.0),
            Coord3D::new(0.0, 100.0, 0.0),
        ];

        assert!(!is_group_too_spread(&positions_tight, 50.0));
        assert!(is_group_too_spread(&positions_spread, 50.0));
    }

    #[test]
    fn test_formation_offset_with_direction() {
        let config = FormationConfig::default();
        let direction = Coord2D::new(1.0, 0.0); // Moving east

        let offset = calculate_formation_offset_with_direction(
            FormationType::Wedge,
            1,
            5,
            direction,
            &config,
        );

        // Should produce a valid 3D offset
        assert!(offset.z == 0.0); // Z is always 0 for formations
    }

    #[test]
    fn test_formation_rotation() {
        let config = FormationConfig::default();

        // Test that formation rotates with leader facing
        let offset_0 = calculate_formation_offset(FormationType::Wedge, 1, 5, 0.0, &config);

        let offset_90 = calculate_formation_offset(FormationType::Wedge, 1, 5, PI / 2.0, &config);

        // Offsets should be rotated by 90 degrees
        // offset_0 should be roughly (a, b)
        // offset_90 should be roughly (-b, a)
        assert!((offset_0.x + offset_90.y).abs() < 1.0);
        assert!((offset_0.y - offset_90.x).abs() < 1.0);
    }

    #[test]
    fn test_empty_group_center() {
        let positions: Vec<Coord3D> = vec![];
        let center = calculate_group_center(&positions);

        assert_eq!(center.x, 0.0);
        assert_eq!(center.y, 0.0);
        assert_eq!(center.z, 0.0);
    }

    #[test]
    fn test_empty_group_spread() {
        let positions: Vec<Coord3D> = vec![];
        let spread = calculate_group_spread(&positions);

        assert_eq!(spread, 0.0);
    }

    #[test]
    fn test_ring_capacity() {
        assert_eq!(ring_capacity(0), 1); // Leader
        assert_eq!(ring_capacity(1), 6); // First ring
        assert_eq!(ring_capacity(2), 12); // Second ring
        assert_eq!(ring_capacity(3), 18); // Third ring
    }
}
