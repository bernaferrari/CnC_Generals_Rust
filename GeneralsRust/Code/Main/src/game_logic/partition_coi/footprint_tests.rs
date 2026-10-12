//! Original PartitionManager.cpp row/sample order and receiving-map bounds.
use super::*;

fn shape(major: f32, minor: f32, angle: f32, small: bool, boxed: bool) -> HostPartitionFootprint {
    HostPartitionFootprint {
        major_radius: major,
        minor_radius: minor,
        angle,
        is_small: small,
        is_box: boxed,
    }
}

#[test]
fn clipped_circle_retains_cpp_scanline_first_visit_order() {
    // CPP :1810-1840, cellRadius=2, centre cell (0,0). Off-map
    // getCellAt calls admit nothing; each surviving COI retains its first visit.
    assert_eq!(
        cells_touched_for_footprint_clipped(
            [0.0; 2],
            [2, 2],
            20.0,
            20.0,
            shape(80.0, 80.0, 0.0, false, false)
        )
        .unwrap(),
        vec![(0, 1), (0, 0), (1, 0), (1, 1)]
    );
}

#[test]
fn clipped_small_and_box_retain_cpp_edge_admission_order() {
    assert_eq!(
        cells_touched_for_footprint_clipped(
            [0.0; 2],
            [2, 2],
            0.0,
            0.0,
            shape(20.0, 20.0, 0.0, true, false)
        )
        .unwrap(),
        vec![(0, 0)]
    );
    assert_eq!(
        cells_touched_for_footprint_clipped(
            [0.0; 2],
            [2, 2],
            20.0,
            20.0,
            shape(40.0, 40.0, 0.0, false, true)
        )
        .unwrap(),
        vec![(0, 1), (1, 1), (0, 0), (1, 0)]
    );
}

#[test]
fn clipped_rotated_box_applies_map_origin_after_original_corner_arithmetic() {
    assert_eq!(
        cells_touched_for_footprint_clipped(
            [160.0, -320.0],
            [2, 3],
            240.0,
            -240.0,
            shape(25.0, 12.0, 0.25, false, true)
        )
        .unwrap(),
        vec![(1, 2), (1, 1)]
    );
}

#[test]
fn cpp_zero_box_steps_produce_no_cois_even_when_the_center_is_in_bounds() {
    // :1781-1799 has no forced minimum iteration and no centre fallback.
    for (major, minor) in [(0.0, 40.0), (40.0, 0.0), (0.0, 0.0)] {
        let fp = shape(major, minor, 0.25, false, true);
        assert!(cells_touched_for_footprint(80.0, 80.0, fp).is_empty());
        assert!(
            cells_touched_for_footprint_clipped([0.0; 2], [8, 8], 80.0, 80.0, fp)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn clipped_large_circle_allocates_only_the_receiving_cells_and_off_map_stamps_nothing() {
    let fp = shape(40_000.0, 40_000.0, 0.0, false, false);
    let mut cells = cells_touched_for_footprint_clipped([0.0; 2], [2, 2], 0.0, 0.0, fp).unwrap();
    cells.sort_unstable();
    assert_eq!(cells, vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
    assert!(
        cells_touched_for_footprint_clipped([0.0; 2], [2, 2], -80_000.0, -80_000.0, fp)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn malformed_or_unsupported_footprints_fail_before_enumeration() {
    for fp in [
        shape(f32::NAN, 1.0, 0.0, false, false),
        shape(1.0, -1.0, 0.0, false, true),
        shape(1.0, 1.0, f32::INFINITY, false, true),
        shape(f32::MAX, f32::MAX, 0.0, false, false),
        shape(12_000.0, 12_000.0, 0.0, false, true),
    ] {
        assert!(cells_touched_for_footprint_clipped([0.0; 2], [2, 2], 0.0, 0.0, fp).is_err());
    }
    assert_eq!(
        footprint_sample_cost(shape(10_240.0, 10_240.0, 0.0, false, true)),
        Ok(1 << 20)
    );
    assert!(
        cells_touched_for_footprint_clipped(
            [0.0; 2],
            [2, 2],
            f32::NAN,
            0.0,
            shape(1.0, 1.0, 0.0, true, false)
        )
        .is_err()
    );
}
