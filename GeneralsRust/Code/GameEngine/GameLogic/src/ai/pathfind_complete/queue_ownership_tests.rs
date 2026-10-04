use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

fn request(id: ObjectID, destination_x: f32) -> PathRequest {
    PathRequest {
        object_id: id,
        from: Coord3D::new(10.0, 10.0, 0.0),
        to: Coord3D::new(destination_x, 30.0, 0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    }
}

fn crc_bytes(owner: &crate::ai::Pathfinder) -> Vec<u8> {
    let mut bytes = Vec::new();
    owner.crc_pathfinder(&mut XferSave::new(Cursor::new(&mut bytes), 1));
    bytes
}

#[test]
fn request_queues_belong_to_the_driving_pathfinder_and_reset_independently() {
    // AI.cpp:289/306 owns and resets one Pathfinder. The additional full
    // PathRequest snapshots are a Rust host/test residual, not C++ save state.
    let mut first = crate::ai::Pathfinder::new();
    first.reset_with_size(8, 8);
    first.queue_for_path_request(request(77, 20.0)).unwrap();
    let first_crc = crc_bytes(&first);

    let mut second = crate::ai::Pathfinder::new();
    assert_eq!(crc_bytes(&first), first_crc, "construction is inert");
    second.reset_with_size(8, 8);
    second.queue_for_path_request(request(77, 50.0)).unwrap();
    second.queue_for_path_request(request(78, 60.0)).unwrap();
    first.queue_for_path_request(request(79, 40.0)).unwrap();

    assert_eq!(first.inner.request_queue.len(), 2);
    assert_eq!(second.inner.request_queue.len(), 2);
    assert_eq!(first.inner.request_queue[0].to.x, 20.0);
    assert_eq!(second.inner.request_queue[0].to.x, 50.0);
    assert_eq!(first.inner.request_queue[1].object_id, 79);
    assert_eq!(second.inner.request_queue[1].object_id, 78);

    // A not-yet-ready map must not consume either request or ring entry.
    let second_crc = crc_bytes(&second);
    assert_eq!(first.inner.process_queue(PATHFIND_CELLS_PER_FRAME), 0);
    assert_eq!(first.inner.request_queue.len(), 2);
    first.reset();
    assert!(first.inner.request_queue.is_empty());
    assert!(first.inner.object_path_queue.is_empty());
    assert_eq!(second.inner.request_queue[0].to.x, 50.0);
    assert_eq!(crc_bytes(&second), second_crc);
}

#[test]
fn queued_request_dedup_keeps_first_snapshot_and_fifo_ring_order() {
    // AIPathfind.cpp:5641-5663 dedupes before admitting at the ring tail.
    // The Rust residual keeps the original snapshot for a duplicate ObjectID.
    let mut owner = crate::ai::Pathfinder::new();
    owner.queue_for_path_request(request(77, 20.0)).unwrap();
    owner.queue_for_path_request(request(78, 30.0)).unwrap();
    let before_duplicate = crc_bytes(&owner);
    owner.queue_for_path_request(request(77, 99.0)).unwrap();
    assert_eq!(crc_bytes(&owner), before_duplicate);
    assert_eq!(owner.inner.request_queue.len(), 2);
    assert_eq!(owner.inner.request_queue[0].to.x, 20.0);
    assert_eq!(owner.inner.request_queue[1].to.x, 30.0);
    let ring = &mut owner.inner.object_path_queue;
    assert_eq!(ring.pop_front(), Some(77));
    assert_eq!(ring.pop_front(), Some(78));
    assert!(ring.is_empty());
}

#[test]
fn full_object_ring_rejects_new_snapshot_but_accepts_existing_id() {
    // AIPathfind.cpp:5650-5663 leaves one ring slot empty, and deduplication
    // succeeds even at capacity. A failed ring admission adds no residual.
    let mut owner = crate::ai::Pathfinder::new();
    for id in 1..PATHFIND_QUEUE_LEN as ObjectID {
        owner
            .queue_for_path_request(request(id, id as f32))
            .unwrap();
    }
    let before = crc_bytes(&owner);
    let before_len = owner.inner.request_queue.len();
    assert!(
        owner
            .queue_for_path_request(request(PATHFIND_QUEUE_LEN as ObjectID, 80.0))
            .is_err()
    );
    assert_eq!(owner.inner.request_queue.len(), before_len);
    assert_eq!(crc_bytes(&owner), before);
    owner.queue_for_path_request(request(1, 99.0)).unwrap();
    assert_eq!(owner.inner.request_queue[0].to.x, 1.0);
    assert_eq!(crc_bytes(&owner), before);
}

#[test]
fn queued_runtime_survives_version_only_xfer_without_serializing_snapshots() {
    // AIPathfind.cpp:11085-11093 transfers only version 1. Loading that
    // record must not clear a runtime ring or load Rust residual snapshots.
    let mut owner = crate::ai::Pathfinder::new();
    owner.queue_for_path_request(request(77, 25.0)).unwrap();
    owner
        .queue_for_path_request(request(INVALID_ID, 45.0))
        .unwrap();
    let before = crc_bytes(&owner);
    let mut saved = Vec::new();
    owner.xfer_pathfinder(&mut XferSave::new(Cursor::new(&mut saved), 1));
    assert_eq!(saved, [1]);
    let mut load = XferLoad::new(Cursor::new(saved), 1);
    owner.xfer_pathfinder(&mut load);
    owner.load_post_process_pathfinder();
    assert_eq!(load.bytes_read(), 1);
    assert_eq!(owner.inner.request_queue.len(), 2);
    assert_eq!(owner.inner.request_queue[0].to.x, 25.0);
    assert_eq!(owner.inner.request_queue[1].to.x, 45.0);
    assert_eq!(crc_bytes(&owner), before);
}

#[test]
fn invalid_id_residual_deduplicates_without_admitting_a_ring_entry() {
    // INVALID_ID snapshots have no C++ ObjectID ring counterpart. Preserve
    // the residual's established first-snapshot rule and map-ready skip.
    let mut owner = crate::ai::Pathfinder::new();
    let before = crc_bytes(&owner);
    owner
        .queue_for_path_request(request(INVALID_ID, 25.0))
        .unwrap();
    owner
        .queue_for_path_request(request(INVALID_ID, 75.0))
        .unwrap();
    assert_eq!(owner.inner.request_queue.len(), 1);
    assert_eq!(owner.inner.request_queue[0].to.x, 25.0);
    assert!(owner.inner.object_path_queue.is_empty());
    assert_eq!(crc_bytes(&owner), before);
    assert_eq!(owner.inner.process_queue(PATHFIND_CELLS_PER_FRAME), 0);
    assert_eq!(owner.inner.request_queue.len(), 1);
}

#[test]
fn direct_object_admission_keeps_owner_identity_and_cpp_crc_ring_layout() {
    // AIPathfind.cpp:5641-5663 admits only the object ID, dedupes before tail
    // insertion, and leaves one ring slot empty. CRC at 11062-11066 writes
    // all 512 slots followed by the signed head and tail, without a version.
    let mut first = crate::ai::Pathfinder::new();
    let mut second = crate::ai::Pathfinder::new();
    assert!(first.queue_for_path(77));
    assert!(first.queue_for_path(78));
    assert!(second.queue_for_path(77));
    let before_duplicate = crc_bytes(&first);
    assert!(first.queue_for_path(77));
    assert_eq!(crc_bytes(&first), before_duplicate);
    assert!(first.inner.request_queue.is_empty());
    assert!(second.inner.request_queue.is_empty());

    // Four signed extent coordinates, two bools, obsolete int, ignore ID.
    let ring_offset = 4 * 4 + 2 + 4 + 4;
    let ring_end = ring_offset + PATHFIND_QUEUE_LEN * 4;
    let mut expected = Vec::with_capacity(PATHFIND_QUEUE_LEN * 4 + 8);
    for slot in 0..PATHFIND_QUEUE_LEN {
        let id: ObjectID = match slot {
            0 => 77,
            1 => 78,
            _ => INVALID_ID,
        };
        expected.extend_from_slice(&id.to_le_bytes());
    }
    expected.extend_from_slice(&0i32.to_le_bytes());
    expected.extend_from_slice(&2i32.to_le_bytes());
    assert_eq!(&before_duplicate[ring_offset..ring_end + 8], expected);
    let second_crc = crc_bytes(&second);
    assert_eq!(
        &second_crc[ring_offset..ring_offset + 4],
        &77u32.to_le_bytes()
    );
    assert_eq!(
        &second_crc[ring_end..ring_end + 8],
        &[0, 0, 0, 0, 1, 0, 0, 0]
    );
    first.reset();
    assert_eq!(crc_bytes(&second), second_crc);
}
