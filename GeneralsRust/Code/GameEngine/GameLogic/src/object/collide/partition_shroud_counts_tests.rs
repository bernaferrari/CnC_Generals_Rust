//! C++ PartitionManager.cpp:1240-1343 looker/cover transitions and
//! 1604-1613 footprint counts. Querying must never advance or cache the grid.
use super::*;

const FOOTPRINT: [(i32, i32); 3] = [(0, 0), (1, 0), (2, 0)];

#[test]
fn footprint_counts_mixed_clear_fogged_and_shrouded_cells() {
    let mut grid = PartitionShroudGrid::new();
    grid.add_looker(0, 0, 0);
    grid.add_looker(0, 1, 0);
    grid.remove_looker(0, 1, 0);
    assert_eq!(
        grid.count_cells(0, FOOTPRINT),
        PartitionCellShroudCounts {
            total: 3,
            shrouded: 1,
            fogged: 1,
        }
    );
}

#[test]
fn overlapping_lookers_only_fog_after_the_last_removal() {
    let mut grid = PartitionShroudGrid::new();
    grid.add_looker(0, 0, 0);
    grid.add_looker(0, 0, 0);
    grid.remove_looker(0, 0, 0);
    assert_eq!(grid.count_cells(0, [(0, 0)]).fogged, 0);
    assert_eq!(grid.cell_status(0, 0, 0), CellShroudStatus::Clear);
    grid.remove_looker(0, 0, 0);
    assert_eq!(grid.count_cells(0, [(0, 0)]).fogged, 1);
}

#[test]
fn active_cover_restores_shroud_when_last_looker_leaves() {
    let mut grid = PartitionShroudGrid::new();
    grid.add_looker(0, 0, 0);
    grid.add_shrouder(0, 0, 0);
    assert_eq!(grid.cell_status(0, 0, 0), CellShroudStatus::Clear);
    grid.remove_looker(0, 0, 0);
    assert_eq!(grid.count_cells(0, [(0, 0)]).shrouded, 1);
    grid.remove_shrouder(0, 0, 0);
    // C++ removeShrouder does not change current shroud by itself.
    assert_eq!(grid.count_cells(0, [(0, 0)]).shrouded, 1);
    grid.add_looker(0, 0, 0);
    grid.remove_looker(0, 0, 0);
    assert_eq!(grid.count_cells(0, [(0, 0)]).fogged, 1);
}

#[test]
fn independent_world_grids_keep_same_coordinates_isolated() {
    let mut first = PartitionShroudGrid::new();
    first.add_looker(0, 0, 0);
    let mut second = PartitionShroudGrid::new();
    second.add_looker(1, 0, 0);
    assert_eq!(first.count_cells(0, [(0, 0)]).shrouded, 0);
    assert_eq!(second.count_cells(0, [(0, 0)]).shrouded, 1);
    second.clear();
    assert_eq!(first.count_cells(0, [(0, 0)]).shrouded, 0);
    assert_eq!(second.count_cells(1, [(0, 0)]).shrouded, 1);
}

#[test]
fn movement_and_reset_do_not_reuse_previous_footprint_counts() {
    let mut grid = PartitionShroudGrid::new();
    grid.add_looker(0, 0, 0);
    assert_eq!(grid.count_cells(0, [(0, 0)]).shrouded, 0);
    assert_eq!(grid.count_cells(0, [(1, 0)]).shrouded, 1);
    grid.clear();
    assert_eq!(grid.count_cells(0, [(0, 0)]).shrouded, 1);
    assert_eq!(grid.known_cell_status(0, 0, 0), None);
}

#[test]
fn fallback_only_samples_missing_cells_even_when_primary_is_shrouded() {
    let mut primary = PartitionShroudGrid::new();
    primary.add_looker(1, 0, 0); // Known, but shrouded for player 0.
    primary.add_looker(0, 1, 0);
    let mut fallback = PartitionShroudGrid::new();
    fallback.add_looker(0, 0, 0); // Must not override primary.
    fallback.add_looker(0, 2, 0);
    fallback.remove_looker(0, 2, 0);
    let mut queries = Vec::new();
    let counts = primary.count_cells_with_fallback(0, FOOTPRINT, |x, y| {
        queries.push((x, y));
        fallback.cell_status(0, x, y)
    });
    assert_eq!(queries, [(2, 0)]);
    assert_eq!(
        counts,
        PartitionCellShroudCounts {
            total: 3,
            shrouded: 1,
            fogged: 1
        }
    );
}

#[test]
fn empty_footprint_does_not_query_fallback() {
    let grid = PartitionShroudGrid::new();
    assert_eq!(
        grid.count_cells_with_fallback(0, [], |_, _| panic!("empty COIs")),
        PartitionCellShroudCounts::default()
    );
}

#[test]
fn invalid_players_remain_shrouded_on_known_and_unknown_cells() {
    let mut grid = PartitionShroudGrid::new();
    grid.add_looker(0, 0, 0);
    for player in [-1, 16, i32::MAX] {
        assert_eq!(
            grid.count_cells(player, FOOTPRINT),
            PartitionCellShroudCounts {
                total: 3,
                shrouded: 3,
                fogged: 0
            }
        );
    }
}

#[test]
fn batch_matches_original_per_cell_walk_for_all_players() {
    let mut grid = PartitionShroudGrid::new();
    for player in 0..16 {
        grid.add_looker(player, player, 0);
        if player % 2 == 0 {
            grid.remove_looker(player, player, 0);
        }
    }
    let cells: Vec<_> = (-1..17).map(|x| (x, 0)).collect();
    for player in -1..17 {
        let mut expected = PartitionCellShroudCounts::default();
        for &(x, y) in &cells {
            expected.total += 1;
            match grid.cell_status(player, x, y) {
                CellShroudStatus::Shrouded => expected.shrouded += 1,
                CellShroudStatus::Fogged => expected.fogged += 1,
                CellShroudStatus::Clear => {}
            }
        }
        assert_eq!(grid.count_cells(player, cells.iter().copied()), expected);
    }
}
