//! Group movement rules from C++ AIGroup and AIData.

use glam::Vec2;

use super::PATHFIND_CELL_SIZE_F;

pub const MIN_INFANTRY_FOR_COLUMN: i32 = 3;
// The live formation planner uses three vehicles. The C++ TAiData constructor uses four.
pub const MIN_VEHICLES_FOR_COLUMN: i32 = 3;
pub const DEFAULT_MIN_GROUP_PATH_DISTANCE: f32 = 100.0;
pub const DEFAULT_GROUP_PATH_REQUIRED_DISTANCE: f32 = 500.0;

/// C++ AIGroup::groupMoveToPosition tests the scaled gather area. Its cell cap
/// uses the horizontal span for both axes.
pub fn is_group_gather_target(min: Vec2, max: Vec2, target: Vec2) -> bool {
    const GATHER_FACTOR: f32 = 0.5;

    let center = 0.5 * (min + max);
    let half = 0.5 * (max - min) * GATHER_FACTOR;
    if !(target.cmpge(center - half).all() && target.cmple(center + half).all()) {
        return false;
    }

    let span_cells = (half.x * 2.0 / PATHFIND_CELL_SIZE_F) as i32;
    span_cells.saturating_mul(span_cells) < 2000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_area_keeps_inclusive_edges_and_caps_the_squared_horizontal_span() {
        let min = Vec2::new(-440.0, -2_000.0);
        let max = Vec2::new(440.0, 2_000.0);
        assert!(is_group_gather_target(min, max, Vec2::new(220.0, -1_000.0)));
        assert!(!is_group_gather_target(
            min,
            max,
            Vec2::new(220.25, -1_000.0)
        ));
        assert!(!is_group_gather_target(
            Vec2::new(-450.0, -1.0),
            Vec2::new(450.0, 1.0),
            Vec2::ZERO,
        ));
    }
}
