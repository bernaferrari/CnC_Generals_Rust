use gamelogic::ai::pathfind_complete::PathfindingSystem;

fn process_one(world: &mut PathfindingSystem) -> (usize, usize) {
    let mut calls = 0;
    let count = world
        .process_queue_with_owner(1, None, |_, _| {
            calls += 1;
            Ok(true)
        })
        .unwrap();
    (count, calls)
}

#[test]
fn clearing_passability_does_not_request_a_zone_recalculation() {
    let mut owner = PathfindingSystem::new(20, 20);
    let mut peer = PathfindingSystem::new(20, 20);
    owner.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();

    owner.clear_zone_passable_flags();
    assert!(owner.queue_for_path(1));
    assert!(peer.queue_for_path(1));
    assert_eq!(process_one(&mut owner), process_one(&mut peer));
}

#[test]
fn enabling_passability_does_not_cancel_a_pending_zone_recalculation() {
    let mut owner = PathfindingSystem::new(20, 20);
    let mut peer = PathfindingSystem::new(20, 20);
    owner.new_map_from_classified_cells();
    peer.new_map_from_classified_cells();
    owner.mark_zones_dirty();
    peer.mark_zones_dirty();
    owner.set_all_zone_passable();
    assert!(owner.queue_for_path(1));
    assert!(peer.queue_for_path(1));
    assert_eq!(process_one(&mut owner), process_one(&mut peer));
    assert_eq!(process_one(&mut owner), process_one(&mut peer));
}
