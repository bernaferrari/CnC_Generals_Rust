//! AIPathfind.cpp queueForPath reserves one slot in its 512-entry ring.
use generals_main::game_logic::ObjectId;
use generals_main::game_logic::pathfinding::{PathfindingSystem, PendingHostPath};
use glam::Vec3;

fn request(id: u32) -> PendingHostPath {
    PendingHostPath {
        unit_id: ObjectId(id),
        start: Vec3::ZERO,
        destination: Vec3::new(id as f32, 0.0, 0.0),
        waypoints: Vec::new(),
        follow_path_appends: Vec::new(),
        aircraft: false,
        surfaces: 1,
        is_crusher: false,
        ignore_obstacle: None,
        adjust_destinations: true,
        restore_adjust_on_install: false,
    }
}

#[test]
fn reserved_slot_limits_each_owned_queue_without_evicting_waiters() {
    let mut owner = PathfindingSystem::new(100.0, 100.0);
    let mut peer = PathfindingSystem::new(100.0, 100.0);
    assert!(peer.queue_path(request(1)));
    for id in 1..=511 {
        assert!(owner.queue_path(request(id)));
    }
    assert_eq!(owner.pending_path_count(), 511);
    assert!(
        !owner.queue_path(request(512)),
        "the reserved slot must stay empty"
    );
    assert!(
        owner.queue_path(request(1)),
        "a duplicate remains admissible when full"
    );
    assert_eq!(owner.pop_pending_path().unwrap().unit_id, ObjectId(1));
    assert!(
        owner.queue_path(request(512)),
        "a freed slot must be reusable"
    );
    let ids: Vec<_> = owner
        .take_pending_paths()
        .into_iter()
        .map(|r| r.unit_id.0)
        .collect();
    assert_eq!(ids, (2..=512).collect::<Vec<_>>());
    assert_eq!(peer.pending_path_count(), 1);
    assert_eq!(peer.pop_pending_path().unwrap().unit_id, ObjectId(1));
}
