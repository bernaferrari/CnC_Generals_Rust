//! PartitionManager.cpp:3123-3316 FASTER_GCO visitation; no ambient object lookup.
use super::*;

impl PartitionManager {
    /// Candidate identity order is radius bucket, Y/X offset, then newest COI first.
    /// Callers perform the original live distance and filter observations afterwards.
    pub(crate) fn ordered_search_candidates(
        &self,
        position: &Coord3D,
        range: f32,
    ) -> Vec<ObjectId> {
        let center = CellCoord::from_world_pos(position);
        let max_radius = super::super::partition_coi::world_to_cell_dist(range.max(0.0));
        let mut cells = self
            .cells
            .keys()
            .filter_map(|cell| {
                let dx = cell.x - center.x;
                let dy = cell.y - center.y;
                let gap_x = f64::from((dx.abs() - 1).max(0));
                let gap_y = f64::from((dy.abs() - 1).max(0));
                let radius = (gap_x * gap_x + gap_y * gap_y).sqrt().ceil() as i32;
                (range >= HUGE_DIST || radius <= max_radius).then_some((radius, dy, dx, *cell))
            })
            .collect::<Vec<_>>();
        cells.sort_by_key(|(radius, y, x, _)| (*radius, *y, *x));
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (_, _, _, coord) in cells {
            for id in &self.cells[&coord].objects {
                if seen.insert(*id) {
                    result.push(*id);
                }
            }
        }
        result
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
            partition.ordered_search_candidates(&Coord3D::new(1.0, 1.0, 0.0), PARTITION_CELL_SIZE),
            [71, 4, 90, 8]
        );
        assert!(
            PartitionManager::new()
                .ordered_search_candidates(&Coord3D::new(1.0, 1.0, 0.0), PARTITION_CELL_SIZE)
                .is_empty()
        );
    }
}
