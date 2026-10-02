use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

fn crc_bytes(owner: &crate::ai::Pathfinder) -> Vec<u8> {
    let mut bytes = Vec::new();
    owner.crc_pathfinder(&mut XferSave::new(Cursor::new(&mut bytes), 1));
    bytes
}

#[test]
fn counter_cleanup_crc_and_reset_use_the_driving_pathfinder() {
    // AIPathfind.cpp:3848/4798/5899/11079: reset, released cells,
    // per-frame budget and final CRC integer. CRC observes, Xfer loads.
    let mut first = crate::ai::Pathfinder::new();
    first.reset_with_size(8, 8);
    first.update_goal_cells(
        GridCoord::new(2, 2),
        77,
        PathfindLayerEnum::Ground,
        0,
        true,
        false,
    );
    first.inner.note_open_closed_cells(3, 5);
    first.clean_open_and_closed_lists();
    let before_second = crc_bytes(&first);

    let mut second = crate::ai::Pathfinder::new();
    second.reset_with_size(8, 8);
    second.update_goal_cells(
        GridCoord::new(5, 5),
        77,
        PathfindLayerEnum::Ground,
        0,
        true,
        false,
    );
    second.inner.note_open_closed_cells(11, 9);
    second.clean_open_and_closed_lists();
    assert_eq!(first.inner.cumulative_cells_allocated(), 8);
    assert_eq!(second.inner.cumulative_cells_allocated(), 20);
    assert_eq!(crc_bytes(&first), before_second);
    assert_eq!(
        &before_second[before_second.len() - 4..],
        &8i32.to_le_bytes()
    );
    let second_bytes = crc_bytes(&second);
    assert_eq!(
        &second_bytes[second_bytes.len() - 4..],
        &20i32.to_le_bytes()
    );

    first.clean_open_and_closed_lists(); // lists were already emptied
    assert_eq!(first.inner.cumulative_cells_allocated(), 8);
    assert_eq!(first.inner.process_queue(PATHFIND_CELLS_PER_FRAME), 0); // map not ready
    assert_eq!(first.inner.cumulative_cells_allocated(), 8);
    second.reset();
    assert_eq!(second.inner.cumulative_cells_allocated(), 0);
    assert_eq!(crc_bytes(&first), before_second);
}

#[test]
fn version_only_xfer_does_not_load_or_reset_runtime_counter() {
    // AIPathfind.cpp:11085-11093 transfers one version byte, no state.
    let mut owner = crate::ai::Pathfinder::new();
    owner.reset_with_size(4, 4);
    owner.inner.note_open_closed_cells(4, 6);
    owner.clean_open_and_closed_lists();
    let before = crc_bytes(&owner);
    let mut saved = Vec::new();
    owner.xfer_pathfinder(&mut XferSave::new(Cursor::new(&mut saved), 1));
    assert_eq!(saved, [1]);
    let mut load = XferLoad::new(Cursor::new(saved), 1);
    owner.xfer_pathfinder(&mut load);
    owner.load_post_process_pathfinder();
    assert_eq!(load.bytes_read(), 1);
    assert_eq!(owner.inner.cumulative_cells_allocated(), 10);
    assert_eq!(crc_bytes(&owner), before);
}

#[test]
fn owned_counter_keeps_previous_atomic_wrapping_arithmetic() {
    // Retain the Rust port's defined AtomicI32 arithmetic at extreme counts;
    // this does not claim C++ signed-overflow behavior is defined.
    let mut owner = PathfindingSystem::new(1, 1);
    owner.cumulative_cells_allocated = i32::MAX;
    owner.note_open_closed_cells(1, 0);
    owner.clean_open_and_closed_lists();
    assert_eq!(owner.cumulative_cells_allocated(), i32::MIN);
}
