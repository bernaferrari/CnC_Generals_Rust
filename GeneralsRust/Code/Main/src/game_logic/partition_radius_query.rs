//! Original FASTER_GCO cell-candidate acceptance, not iterator or Object filtering.
//! CPP PartitionManager.cpp3067–3156/3228–3316. No copied pose or index owner.

const HUGE_DIST: f32 = 1_000_000.;

/// One immutable operation over existing owned COIs. Scanning occupied cells
/// gives the original bucket's candidate set; the caller retains its existing
/// sorted-ID policy instead of claiming original radius/offset/COI visit order.
pub(super) struct RadiusCellQuery {
    center: [i32; 2],
    cell_size: f32,
    dimensions: Option<[usize; 2]>,
    maximum_bucket: Option<i32>,
}

impl RadiusCellQuery {
    /// Actual map cell size is already admitted finite and >=1. None dimensions
    /// is the existing standalone40 component domain, not a native map fallback.
    pub(super) fn new(
        center: [i32; 2],
        cell_size: f32,
        dimensions: Option<[usize; 2]>,
        radius: f32,
    ) -> Self {
        let maximum_bucket = if radius < HUGE_DIST {
            // worldToCellDist uses stored Real reciprocal, then Real product.
            let inverse = (1.0_f64 / f64::from(cell_size)) as f32;
            Some(source_ceil_nonnegative(radius * inverse))
        } else {
            // Original HUGE_DIST branch traverses the complete radiusVec.
            None
        };
        Self {
            center,
            cell_size,
            dimensions,
            maximum_bucket,
        }
    }

    /// One accepted RadiusVec bucket and its original y/x insertion order.
    pub(super) fn traversal_key(&self, cell: [i32; 2]) -> Option<(i32, i64, i64)> {
        let offset = self.offset_within_grid(cell)?;
        let bucket = minimum_bucket(offset, self.cell_size);
        self.maximum_bucket
            .is_none_or(|maximum| bucket <= maximum)
            .then_some((bucket, offset[1], offset[0]))
    }

    pub(super) fn accepts(&self, cell: [i32; 2]) -> bool {
        if self.maximum_bucket.is_none() {
            self.offset_within_grid(cell).is_some()
        } else {
            self.traversal_key(cell).is_some()
        }
    }

    fn offset_within_grid(&self, cell: [i32; 2]) -> Option<[i64; 2]> {
        let offset = [
            i64::from(cell[0]) - i64::from(self.center[0]),
            i64::from(cell[1]) - i64::from(self.center[1]),
        ];
        if let Some([width, height]) = self.dimensions {
            // calcRadiusVec enumerates -count+1 .. count (exclusive), even
            // when the query center lies outside the admitted map extent.
            if cell[0] < 0
                || cell[1] < 0
                || i64::from(cell[0]) >= width as i64
                || i64::from(cell[1]) >= height as i64
                || offset[0] <= -(width as i64)
                || offset[0] >= width as i64
                || offset[1] <= -(height as i64)
                || offset[1] >= height as i64
            {
                return None;
            }
        }
        Some(offset)
    }
}

/// Literal calcMinRadius observations: Real corners/differences; double sums;
/// sqrtf rounds its double input to Real and returns Real; double divide;
/// REAL_TO_INT_CEIL first converts that quotient to its float parameter.
fn minimum_bucket(offset: [i64; 2], cell_size: f32) -> i32 {
    let half = cell_size * 0.5;
    // Every X corner difference can pair with every Y difference. Preserve
    // the C++ Real subtractions before choosing each minimum and squaring
    // in double precision. The original finite sentinel also caps the sum.
    let separation = |axis: i64| {
        let center = axis as f32 * cell_size;
        let lower = center - half;
        let upper = center + half;
        (-half - lower)
            .abs()
            .min((-half - upper).abs())
            .min((half - lower).abs())
            .min((half - upper).abs())
    };
    let dx = f64::from(separation(offset[0]));
    let dy = f64::from(separation(offset[1]));
    let minimum_squared = (dx * dx + dy * dy).min(1.0e12_f64);
    let distance = f64::from((minimum_squared as f32).sqrt());
    source_ceil_nonnegative((distance / f64::from(cell_size)) as f32)
}

/// BaseType.h:204-226 `REAL_TO_INT_CEIL`, shared with every source conversion.
fn source_ceil_nonnegative(value: f32) -> i32 {
    game_engine::base_type::real_to_int_ceil(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_almost_one_addition_preserves_integral_rounding_trap() {
        for (input, expected) in [(0., 0), (0.375, 1), (1., 2), (1.5, 2), (2., 3), (3., 4)] {
            assert_eq!(source_ceil_nonnegative(input), expected);
        }
        assert_ne!(source_ceil_nonnegative(1.), 1_f32.ceil() as i32);
    }

    #[test]
    fn literal_cpp_corner_buckets_admit_adjacent_hull_without_circle_stamp() {
        for offset in [[0, 0], [-1, 0], [1, 0], [-1, -1], [1, 1]] {
            assert_eq!(minimum_bucket(offset, 40.), 0);
        }
        for offset in [[2, 0], [-2, 0], [2, 1], [2, 2]] {
            assert_eq!(minimum_bucket(offset, 40.), 2);
        }
        assert_eq!(minimum_bucket([3, 0], 40.), 3);
        // Actual retained large-box fixture queries(80,0), radius15, grid40.
        let query = RadiusCellQuery::new([2, 0], 40., None, 15.);
        assert!(query.accepts([1, 0]));
        assert!(query.accepts([1, -1]));
        assert!(!query.accepts([0, 0]));
        assert!(RadiusCellQuery::new([2, 0], 40., None, 40.).accepts([0, 0]));
        // Scaling both worlds leaves the original relative cell buckets.
        assert!(RadiusCellQuery::new([2, 0], 64., Some([7, 7]), 24.).accepts([1, 0]));
        assert!(!RadiusCellQuery::new([2, 0], 64., Some([7, 7]), 24.).accepts([0, 0]));
    }

    #[test]
    fn actual_grid_radius_vec_bounds_apply_even_to_huge_offmap_queries() {
        let query = RadiusCellQuery::new([-1, 0], 64., Some([3, 3]), HUGE_DIST);
        assert!(query.accepts([0, 0])); // offset1 is in -2..=2
        assert!(query.accepts([1, 0])); // offset2 remains in radiusVec
        assert!(!query.accepts([2, 0])); // offset3 was never in radiusVec
        assert!(!query.accepts([-1, 0])); // not an admitted map cell
        assert!(!query.accepts([0, 3]));
    }
}
