//! Preserve the C++ per-player seen history while removing the unused status
//! cache. No player, object-registry, or partition global is needed by this step.
use super::*;

const CLEAR: PartitionCellShroudCounts = PartitionCellShroudCounts {
    total: 1,
    shrouded: 0,
    fogged: 0,
};
const FOGGED: PartitionCellShroudCounts = PartitionCellShroudCounts {
    total: 1,
    shrouded: 0,
    fogged: 1,
};
const SHROUDED: PartitionCellShroudCounts = PartitionCellShroudCounts {
    total: 1,
    shrouded: 1,
    fogged: 0,
};

fn status(
    data: &PartitionData,
    player: usize,
    counts: PartitionCellShroudCounts,
    immobile: bool,
    mine: bool,
    relationship: Relationship,
) -> ObjectShroudStatus {
    data.status_from_counts(player, counts, immobile, mine, || Some(relationship))
}

#[test]
fn enemy_building_fog_requires_seen_history_and_cover_forgets_it() {
    let data = PartitionData::new();
    let query = |cells| status(&data, 0, cells, true, false, Relationship::Enemies);
    assert_eq!(query(FOGGED), ObjectShroudStatus::Shrouded);
    assert_eq!(query(CLEAR), ObjectShroudStatus::Clear);
    assert_eq!(query(FOGGED), ObjectShroudStatus::Fogged);
    assert_eq!(query(SHROUDED), ObjectShroudStatus::Shrouded);
    assert_eq!(query(FOGGED), ObjectShroudStatus::Shrouded);
}

#[test]
fn neutral_immobile_can_be_fogged_without_prior_sighting() {
    let data = PartitionData::new();
    assert_eq!(
        status(&data, 0, FOGGED, true, false, Relationship::Neutral),
        ObjectShroudStatus::Fogged
    );
    assert_eq!(
        status(&data, 0, FOGGED, false, false, Relationship::Neutral),
        ObjectShroudStatus::Shrouded
    );
}

#[test]
fn enemy_mobile_objects_and_mines_do_not_remain_visible_in_fog() {
    for (immobile, mine) in [(false, false), (true, true)] {
        let data = PartitionData::new();
        assert_eq!(
            status(&data, 0, CLEAR, immobile, mine, Relationship::Enemies),
            ObjectShroudStatus::Clear
        );
        assert_eq!(
            status(&data, 0, FOGGED, immobile, mine, Relationship::Enemies),
            ObjectShroudStatus::Shrouded
        );
    }
}

#[test]
fn partial_clear_marks_seen_and_empty_footprint_forgets_it() {
    let data = PartitionData::new();
    let partial = PartitionCellShroudCounts {
        total: 2,
        shrouded: 1,
        fogged: 0,
    };
    let query = |cells| status(&data, 0, cells, true, false, Relationship::Enemies);
    assert_eq!(query(partial), ObjectShroudStatus::PartialClear);
    assert_eq!(query(FOGGED), ObjectShroudStatus::Fogged);
    assert_eq!(
        query(PartitionCellShroudCounts::default()),
        ObjectShroudStatus::Shrouded
    );
    assert_eq!(query(FOGGED), ObjectShroudStatus::Shrouded);
}

#[test]
fn seen_history_is_separate_for_players_and_instances() {
    let first = PartitionData::new();
    let second = PartitionData::new();
    assert_eq!(
        status(&first, 0, CLEAR, true, false, Relationship::Enemies),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        status(&second, 1, CLEAR, true, false, Relationship::Enemies),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        status(&first, 0, FOGGED, true, false, Relationship::Enemies),
        ObjectShroudStatus::Fogged
    );
    assert_eq!(
        status(&first, 1, FOGGED, true, false, Relationship::Enemies),
        ObjectShroudStatus::Shrouded
    );
    assert_eq!(
        status(&second, 0, FOGGED, true, false, Relationship::Enemies),
        ObjectShroudStatus::Shrouded
    );
    assert_eq!(
        status(&second, 1, FOGGED, true, false, Relationship::Enemies),
        ObjectShroudStatus::Fogged
    );
}

#[test]
fn clear_and_fully_shrouded_queries_do_not_discover_relationships() {
    let data = PartitionData::new();
    assert_eq!(
        data.status_from_counts(0, CLEAR, true, false, || panic!("clear query")),
        ObjectShroudStatus::Clear
    );
    assert_eq!(
        data.status_from_counts(0, SHROUDED, true, false, || panic!("shrouded query")),
        ObjectShroudStatus::Shrouded
    );
}

/// Run alone with `--ignored --nocapture --test-threads=1`. This measures the
/// actual compatibility adapters, not frame rate or full simulation cost.
#[test]
#[ignore = "manual footprint-query comparison"]
fn benchmark_partition_footprint_queries() {
    use crate::object::collide::partition_manager::PartitionManager;
    use std::hint::black_box;
    use std::time::Instant;

    struct RestorePartition(Option<PartitionManager>);
    impl Drop for RestorePartition {
        fn drop(&mut self) {
            *PARTITION_MANAGER.write().unwrap() = self.0.take().unwrap();
        }
    }
    let _restore = {
        let mut primary = PARTITION_MANAGER.write().unwrap();
        let original = std::mem::replace(&mut *primary, PartitionManager::new());
        for x in 0..64 {
            match x % 4 {
                0 => primary.shroud.add_looker(0, x, 0),
                1 => primary.shroud.add_looker(1, x, 0),
                2 => {
                    primary.shroud.add_looker(0, x, 0);
                    primary.shroud.remove_looker(0, x, 0);
                }
                _ => {} // Missing primary cells exercise the live fallback.
            }
        }
        RestorePartition(Some(original))
    };
    let cells: Vec<_> = (0..64).map(|x| (x, 0)).collect();
    let per_cell = || {
        PartitionCellShroudCounts::sample(cells.iter().copied(), |x, y| {
            partition_cell_shroud_status(0, x, y)
        })
    };
    let expected = per_cell(); // Warm up the existing GameLogic singleton.
    assert_eq!(partition_cell_shroud_counts(0, &cells), expected);
    for trial in 0..3 {
        let start = Instant::now();
        for _ in 0..20_000 {
            assert_eq!(black_box(per_cell()), expected);
        }
        let individual = start.elapsed();
        let start = Instant::now();
        for _ in 0..20_000 {
            assert_eq!(black_box(partition_cell_shroud_counts(0, &cells)), expected);
        }
        let batched = start.elapsed();
        println!("trial {trial}: 20,000 x 64 cells: per-cell={individual:?}, batch={batched:?}");
    }
}
