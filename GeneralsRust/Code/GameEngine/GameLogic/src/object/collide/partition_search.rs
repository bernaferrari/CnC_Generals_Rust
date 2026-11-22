//! PartitionManager.cpp:3123-3316 FASTER_GCO visitation; no ambient object lookup.
use super::*;

impl PartitionManager {
    /// C++ `getClosestObjects` cell ordering: cells sorted by radius bucket,
    /// then Y/X offset, matching `m_radiusVec` ring visitation under
    /// FASTER_GCO.
    fn sorted_search_cells(
        &self,
        position: &Coord3D,
        range: f32,
    ) -> Vec<(i32, i32, i32, CellCoord)> {
        let center = CellCoord::from_world_pos(position);
        let max_radius = super::super::partition_coi::world_to_cell_dist(range.max(0.0));
        let cell_size = f64::from(PARTITION_CELL_SIZE);
        let mut cells = self
            .cells
            .keys()
            .filter_map(|cell| {
                let dx = cell.x - center.x;
                let dy = cell.y - center.y;
                let gap_x = f64::from((dx.abs() - 1).max(0)) * cell_size;
                let gap_y = f64::from((dy.abs() - 1).max(0)) * cell_size;
                let gap = ((gap_x * gap_x + gap_y * gap_y) as f32).sqrt();
                // calcMinRadius uses sqrtf before the original REAL_TO_INT_CEIL.
                let radius =
                    game_engine::base_type::real_to_int_ceil((f64::from(gap) / cell_size) as f32);
                (range >= HUGE_DIST || radius <= max_radius).then_some((radius, dy, dx, *cell))
            })
            .collect::<Vec<_>>();
        cells.sort_by_key(|(radius, y, x, _)| (*radius, *y, *x));
        cells
    }

    /// Candidate identity order is radius bucket, Y/X offset, then newest COI first.
    /// Callers perform the original live distance and filter observations afterwards.
    pub(crate) fn ordered_search_candidates(
        &self,
        position: &Coord3D,
        range: f32,
    ) -> Vec<ObjectId> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (_, _, _, coord) in self.sorted_search_cells(position, range) {
            for id in &self.cells[&coord].objects {
                if seen.insert(*id) {
                    result.push(*id);
                }
            }
        }
        result
    }

    /// C++ `getClosestObject` under FASTER_GCO (PartitionManager.cpp:3245-3315):
    /// rings are visited innermost-first, each cell's COI list head-first with
    /// done-flag dedup. `accept(id, best_distance)` returns `Some(distance)`
    /// only for a strictly-closer allowed candidate; the first ring that
    /// accepts one ends the search after that ring finishes.
    pub(crate) fn closest_ordered_candidate(
        &self,
        position: &Coord3D,
        range: f32,
        mut accept: impl FnMut(ObjectId, f32) -> Option<f32>,
    ) -> Option<ObjectId> {
        let mut seen = HashSet::new();
        let mut best: Option<(ObjectId, f32)> = None;
        let mut accepted_ring = None;
        for (radius, _, _, coord) in self.sorted_search_cells(position, range) {
            if accepted_ring.is_some_and(|ring| ring != radius) {
                break;
            }
            for id in &self.cells[&coord].objects {
                if !seen.insert(*id) {
                    continue;
                }
                let best_distance = best.map_or(range * range, |(_, distance)| distance);
                if let Some(distance) = accept(*id, best_distance) {
                    best = Some((*id, distance));
                    accepted_ring = Some(radius);
                }
            }
        }
        best.map(|(id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpp_coi_head_order_survives_removal_without_id_sorting() {
        let mut cell = PartitionCell::new();
        cell.add(91);
        cell.add(3);
        cell.add(70);
        cell.add(3);
        assert_eq!(cell.objects, [70, 3, 91]);
        assert!(cell.remove(3));
        assert_eq!(cell.objects, [70, 91]);
        cell.add(3);
        assert_eq!(cell.objects, [3, 70, 91]);
    }
    #[test]
    fn receiving_query_keeps_cpp_radius_offsets_and_coi_order() {
        let mut partition = PartitionManager::new();
        for (coord, ids) in [
            (CellCoord { x: 0, y: 0 }, vec![90, 4]),
            (CellCoord { x: -1, y: -1 }, vec![71]),
            (CellCoord { x: -2, y: -2 }, vec![6]),
            (CellCoord { x: -2, y: -1 }, vec![15]),
            (CellCoord { x: 2, y: 0 }, vec![8]),
        ] {
            let mut cell = PartitionCell::new();
            for id in ids {
                cell.add(id);
            }
            partition.cells.insert(coord, cell);
        }
        assert_eq!(
            partition.ordered_search_candidates(&Coord3D::new(1.0, 1.0, 0.0), 0.0),
            [71, 4, 90]
        );
        assert_eq!(
            partition.ordered_search_candidates(
                &Coord3D::new(1.0, 1.0, 0.0),
                PARTITION_CELL_SIZE - 1.0,
            ),
            [71, 4, 90]
        );
        assert_eq!(
            partition.ordered_search_candidates(&Coord3D::new(1.0, 1.0, 0.0), PARTITION_CELL_SIZE),
            [71, 4, 90, 6, 15, 8]
        );
        assert!(
            PartitionManager::new()
                .ordered_search_candidates(&Coord3D::new(1.0, 1.0, 0.0), PARTITION_CELL_SIZE)
                .is_empty()
        );
    }
    #[test]
    fn closest_ordered_candidate_uses_strict_distance_then_first_coi() {
        let geometry =
            crate::object::collide::collision_geometry::GeometryInfo::new_sphere(1.0, false);
        let mut partition = PartitionManager::new();
        // Registration order deliberately puts the far and +x objects first;
        // cell visitation (dx -1 before +1 at the same ring) decides ties.
        partition
            .register_object(7, Coord3D::new(200.0, 0.0, 0.0), geometry)
            .unwrap();
        partition
            .register_object(9, Coord3D::new(50.0, 0.0, 0.0), geometry)
            .unwrap();
        partition
            .register_object(4, Coord3D::new(-20.0, 0.0, 0.0), geometry)
            .unwrap();
        let position = Coord3D::new(0.0, 0.0, 0.0);
        // Equal-distance tie: the -x cell visits first at the same ring and
        // keeps the win; the far ring is never reached once a ring accepts.
        assert_eq!(
            partition.closest_ordered_candidate(&position, 300.0, |id, best| {
                let distance = match id {
                    4 | 9 => 1600.0,
                    7 => 14400.0,
                    _ => f32::MAX,
                };
                (distance < best).then_some(distance)
            }),
            Some(4)
        );
        // Strict improvement still applies inside the ring: the later-visited
        // +x candidate wins when its distance is strictly smaller.
        assert_eq!(
            partition.closest_ordered_candidate(&position, 300.0, |id, best| {
                let distance = match id {
                    4 => 2500.0,
                    9 => 100.0,
                    7 => 14400.0,
                    _ => f32::MAX,
                };
                (distance < best).then_some(distance)
            }),
            Some(9)
        );
        // Strict boundary: a distance equal to range² is never closer.
        assert_eq!(
            partition.closest_ordered_candidate(&position, 40.0, |id, best| {
                let distance = match id {
                    4 | 9 => 1600.0,
                    _ => f32::MAX,
                };
                (distance < best).then_some(distance)
            }),
            None
        );
    }
}
