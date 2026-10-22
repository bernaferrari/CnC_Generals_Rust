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

    pub(super) fn accepts(&self, cell: [i32; 2]) -> bool {
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
                return false;
            }
        }
        self.maximum_bucket
            .is_none_or(|maximum| minimum_bucket(offset, self.cell_size) <= maximum)
    }
}

/// Literal calcMinRadius observations: Real corners/differences; double sums;
/// sqrtf rounds its double input to Real and returns Real; double divide;
/// REAL_TO_INT_CEIL first converts that quotient to its float parameter.
fn minimum_bucket(offset: [i64; 2], cell_size: f32) -> i32 {
    let half = cell_size * 0.5;
    let center = [[-half, -half], [half, -half], [-half, half], [half, half]];
    let x = offset[0] as f32 * cell_size;
    let y = offset[1] as f32 * cell_size;
    let other = [
        [x - half, y - half],
        [x + half, y - half],
        [x - half, y + half],
        [x + half, y + half],
    ];
    // Preserve the original finite sentinel, including its large-distance cap.
    let mut minimum_squared = 1.0e12_f64;
    for a in center {
        for b in other {
            let dx = f64::from(a[0] - b[0]);
            let dy = f64::from(a[1] - b[1]);
            let squared = dx * dx + dy * dy;
            if minimum_squared > squared {
                minimum_squared = squared;
            }
        }
    }
    let distance = f64::from((minimum_squared as f32).sqrt());
    source_ceil_nonnegative((distance / f64::from(cell_size)) as f32)
}

/// BaseType.h204/225 fast_float_ceil + fast_float_trunc, followed by signed32
/// FISTP. This kernel only supplies finite nonnegative values <=1,000,000:
/// maxDist<HUGE_DIST with admitted size>=1, or sqrtf(1e12)/size. The truncated
/// float is integral and exactly representable by i32, so NEAR conversion is
/// identical to this cast. No out-of-range x87 or physical-hardware claim.
fn source_ceil_nonnegative(value: f32) -> i32 {
    let adjusted = value + f32::from_bits(0x3f7f_ffff);
    adjusted.trunc() as i32
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
