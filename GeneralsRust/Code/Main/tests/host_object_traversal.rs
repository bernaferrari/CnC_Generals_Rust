//! Exercise the same ordering helper used by the owned host object store.
#[path = "../src/game_logic/game_logic/host_object_store/registered_order.rs"]
mod registered_order;
use std::collections::HashMap;

#[test]
fn mutable_traversal_follows_registration_lifetimes_and_isolates_worlds() {
    let mut world = HashMap::from([(91, 0), (4, 0), (700, 0), (23, 0), (999, 7)]);
    let mut peer = world.clone();
    // 999 is lookup-visible but its constructor has not registered it yet.
    let order = [4, 23, 700, 91];
    let seen: Vec<_> = registered_order::registered_mut(&mut world, &order)
        .map(|(&id, value)| {
            *value = id;
            id
        })
        .collect();
    assert_eq!(seen, order);
    assert_eq!(world[&999], 7);
    assert_eq!(peer[&4], 0);
    assert_eq!(world[&4], 4);
    // A removal followed by a new lifetime prepends its ID to the list.
    peer.remove(&23);
    peer.insert(23, 11);
    let recreated = [23, 91, 700, 4];
    let seen: Vec<_> = registered_order::registered_mut(&mut peer, &recreated)
        .map(|(&id, _)| id)
        .collect();
    assert_eq!(seen, recreated);
    assert_eq!(world[&23], 23);
}
