//! Original PartitionManager.cpp:3855-3934 ring order, with candidate
//! admission borrowed from the receiving terrain/pathfinder/partition owner.
use crate::common::{Coord3D, Real};

pub(crate) fn find_position_around_candidates(
    center: Coord3D,
    min_radius: Real,
    max_radius: Real,
    start_angle: Real,
    mut admit: impl FnMut(Coord3D, Real) -> Option<Coord3D>,
) -> Option<Coord3D> {
    const RING_SPACING: Real = 5.0;
    const TWO_PI: Real = std::f32::consts::PI * 2.0;
    let mut dist = min_radius;
    while dist <= max_radius {
        let spacing = if dist == min_radius {
            TWO_PI
        } else {
            (RING_SPACING / (dist + 1.0)) * (TWO_PI / 6.0)
        };
        let samples = ((TWO_PI / spacing) / 2.0).ceil() as i32;
        for sample in 0..samples {
            let offset = spacing * sample as Real;
            let angle = start_angle + offset;
            let position = Coord3D::new(
                center.x + dist * angle.cos(),
                center.y + dist * angle.sin(),
                center.z,
            );
            if let Some(position) = admit(position, angle) {
                return Some(position);
            }
            if sample != 0 {
                let angle = start_angle - offset;
                let position = Coord3D::new(
                    center.x + dist * angle.cos(),
                    center.y + dist * angle.sin(),
                    center.z,
                );
                if let Some(position) = admit(position, angle) {
                    return Some(position);
                }
            }
        }
        dist += RING_SPACING;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dozer_sphere_ring_admits_exact_approach_candidate_and_receiving_height() {
        let center = Coord3D::new(20.0, 40.0, 90.0);
        let source = Coord3D::new(20.0, 60.0, 12.0);
        let start = (source.y - center.y).atan2(source.x - center.x);
        let mut checked = Vec::new();
        let found =
            find_position_around_candidates(center, 10.0, 10.0, start, |mut position, angle| {
                checked.push((position, angle));
                position.z = 17.0;
                Some(position)
            })
            .unwrap();
        assert_eq!(checked.len(), 1);
        assert_eq!(checked[0].1, start);
        assert!((found.x - 20.0).abs() < 0.0001);
        assert!((found.y - 50.0).abs() < 0.0001);
        assert_eq!(found.z, 17.0);
    }

    #[test]
    fn rejected_fixed_radius_has_no_other_side_or_ring_before_dozer_fallback() {
        let target = Coord3D::new(11.0, 29.0, 47.0);
        let mut checked = 0;
        let found = find_position_around_candidates(target, 12.0, 12.0, 0.3, |_, _| {
            checked += 1;
            None
        });
        assert_eq!(checked, 1);
        assert!(found.is_none());
        // Actual derived caller uses this fallback when Partition rejects.
        assert_eq!(found.unwrap_or(target), target);
    }

    #[test]
    fn outer_ring_preserves_left_then_right_sampling_and_first_admission() {
        let center = Coord3D::new(0.0, 0.0, 47.0);
        let mut checked = Vec::new();
        let result = find_position_around_candidates(center, 0.0, 5.0, 0.7, |position, angle| {
            checked.push((
                ((position.x - center.x).powi(2) + (position.y - center.y).powi(2)).sqrt(),
                angle,
            ));
            (checked.len() == 4).then_some(position)
        })
        .unwrap();
        let spacing = (5.0 / 6.0) * (std::f32::consts::TAU / 6.0);
        assert_eq!(checked.len(), 4);
        assert_eq!(checked[0], (0.0, 0.7));
        assert!((checked[1].0 - 5.0).abs() < 0.0001);
        assert_eq!(checked[1].1, 0.7);
        assert!((checked[2].1 - (0.7 + spacing)).abs() < 0.0001);
        assert!((checked[3].1 - (0.7 - spacing)).abs() < 0.0001);
        assert!((result.x - 5.0 * (0.7 - spacing).cos()).abs() < 0.0001);
        assert_eq!(result.z, 47.0);
    }
}
