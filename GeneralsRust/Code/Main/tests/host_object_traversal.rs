//! Exercise registered traversal through the live simulation roster.
use generals_game_domain::ObjectId;
use generals_sim::object::{ObjectIdentity, ObjectRoster, RosterEntry};
use std::sync::Arc;

#[derive(Debug)]
struct Actor {
    id: ObjectId,
    token: Arc<()>,
    value: u32,
}

impl Actor {
    fn new(id: u32, value: u32) -> Self {
        Self {
            id: ObjectId(id),
            token: Arc::new(()),
            value,
        }
    }
}

impl RosterEntry for Actor {
    fn roster_identity(&self) -> ObjectIdentity<'_> {
        ObjectIdentity {
            id: self.id,
            generation: 1,
            token: &self.token,
        }
    }
}

#[test]
fn mutable_traversal_follows_registration_lifetimes_and_isolates_worlds() {
    let world_id = Arc::new(());
    let peer_id = Arc::new(());
    let mut world = ObjectRoster::new_for_world(&world_id);
    let mut peer = ObjectRoster::new_for_world(&peer_id);
    for id in [91, 700, 23, 4] {
        world.admit(ObjectId(id), Actor::new(id, 0));
    }
    for id in [23, 4, 700, 91] {
        peer.admit(ObjectId(id), Actor::new(id, 0));
    }

    // 999 is lookup-visible but its constructor has not registered it yet.
    let receipt = world.admit_constructor_lookup(Actor::new(999, 7)).unwrap();
    let seen: Vec<_> = world
        .registered_mut()
        .map(|(id, actor)| {
            actor.value = id.0;
            id.0
        })
        .collect();
    assert_eq!(seen, [4, 23, 700, 91]);
    assert_eq!(world.lookup(&ObjectId(999)).unwrap().value, 7);
    assert_eq!(peer.lookup(&ObjectId(4)).unwrap().value, 0);
    assert_eq!(world.lookup(&ObjectId(4)).unwrap().value, 4);

    // A removal followed by a new lifetime prepends its ID to the list.
    assert_eq!(peer.remove_owned(&ObjectId(23)).unwrap().value, 0);
    peer.admit(ObjectId(23), Actor::new(23, 11));
    let seen: Vec<_> = peer.registered_mut().map(|(id, _)| id.0).collect();
    assert_eq!(seen, [23, 91, 700, 4]);
    assert_eq!(world.lookup(&ObjectId(23)).unwrap().value, 23);

    world.register_constructor_lookup(&receipt).unwrap();
    assert_eq!(world.ordered_ids()[0], ObjectId(999));
    assert!(peer.lookup(&ObjectId(999)).is_none());
}
