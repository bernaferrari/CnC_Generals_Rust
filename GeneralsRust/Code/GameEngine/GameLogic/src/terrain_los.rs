//! Terrain line-of-sight compatibility exports.

pub use generals_sim::terrain::{LOS_FUDGE, is_clear_line_of_sight};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{Coord3D, MAP_HEIGHT_SCALE};

    #[test]
    fn empty_map_is_blocked_like_cpp_null_heightmap() {
        let a = Coord3D::new(0.0, 0.0, 10.0);
        let b = Coord3D::new(40.0, 0.0, 10.0);
        assert!(!is_clear_line_of_sight(&a, &b, &[], 0, 0, 0, 10.0));
    }

    #[test]
    fn ridge_blocks_when_max_of_four_corners_exceeds_fudge() {
        // 3x2 cells so the walk can sample cell (0,0) and (1,0).
        let mut data = vec![0u8; 3 * 2];
        // Raise the four corners of cell (1,0) well above the LOS line.
        data[1] = 80;
        data[2] = 80;
        data[1 + 3] = 80;
        data[2 + 3] = 80;
        let a = Coord3D::new(0.0, 0.0, 1.0);
        let b = Coord3D::new(20.0, 0.0, 1.0);
        assert!(!is_clear_line_of_sight(
            &a,
            &b,
            &data,
            3,
            2,
            0,
            80.0 * MAP_HEIGHT_SCALE
        ));
    }

    #[test]
    fn small_bump_under_half_unit_does_not_block() {
        let mut data = vec![0u8; 3 * 2];
        // MAP_HEIGHT_SCALE is 0.625; a single-step of 0 is under 0.5 fudge.
        data[1] = 0;
        let a = Coord3D::new(0.0, 0.0, 1.0);
        let b = Coord3D::new(20.0, 0.0, 1.0);
        assert!(is_clear_line_of_sight(&a, &b, &data, 3, 2, 0, 10.0));
    }
}
