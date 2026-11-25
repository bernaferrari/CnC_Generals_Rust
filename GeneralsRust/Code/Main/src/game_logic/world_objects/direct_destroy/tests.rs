use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

const SPAWN: &str = "
 Behavior = SpawnBehavior Spawn
  OneShot = Yes
  SpawnNumber = 1
  SpawnTemplateName = OrderedChild
  SpawnedRequireSpawner = Yes
 End
";
const TRANSPORT: &str = "
 Behavior = TransportContain Transport
  Slots = 1
  AllowInsideKindOf = INFANTRY
 End
";

fn world(spawn_first: bool) -> GameLogic {
    let (first, second) = if spawn_first {
        (SPAWN, TRANSPORT)
    } else {
        (TRANSPORT, SPAWN)
    };
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "delete-order.ini".into(),
            format!(
                r"
Object OrderedCarrier
 KindOf = VEHICLE
 Body = ActiveBody Body
  MaxHealth = 500
  InitialHealth = 500
 End
 {first}
 {second}
 Behavior = DefaultProductionExitUpdate Exit
  UnitCreatePoint = X:0 Y:0 Z:0
  NaturalRallyPoint = X:0 Y:0 Z:0
 End
End
Object OrderedChild
 KindOf = VEHICLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object OrderedRider
 KindOf = INFANTRY
 TransportSlotCount = 1
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
"
            ),
        )])
        .unwrap();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    world
}

fn activate(world: &mut GameLogic) -> (ObjectId, ObjectId, ObjectId) {
    let position = Vec3::new(50.0, 0.0, 50.0);
    let parent = world
        .create_object_for_player("OrderedCarrier", 1, position)
        .unwrap();
    for _ in 0..32 {
        world.update();
    }
    let roster = world.host_object(parent).unwrap().spawn_behavior_children();
    let [(spawn, _)] = roster.as_slice() else {
        panic!("the authored spawner created exactly one dependent child");
    };
    let rider = world
        .create_object_for_player("OrderedRider", 1, position)
        .unwrap();
    assert!(world.host_object_mut(parent).unwrap().add_occupant(rider));
    world.set_fixture_object_contained_by(rider, Some(parent));
    (parent, *spawn, rider)
}

#[test]
fn authored_delete_callbacks_queue_nested_children_in_behavior_order() {
    // Object::onDestroy visits the roster, and GameLogic::destroyObject appends
    // each object before invoking its own delete callbacks.
    let mut receiver = world(true);
    let mut peer = world(false);
    let (parent, spawn, rider) = activate(&mut receiver);
    assert_eq!(activate(&mut peer), (parent, spawn, rider));

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&receiver).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world(true);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for world in [&mut receiver, &mut restored] {
        world.destroy_object(parent).unwrap();
        world.destroy_object(parent).unwrap();
        let queue: Vec<_> = world
            .objects_to_destroy
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(
            queue,
            [parent, spawn, rider],
            "SpawnBehavior runs before TransportContain"
        );
        world.process_destroy_list();
        assert!(
            [parent, spawn, rider]
                .into_iter()
                .all(|id| world.host_object(id).is_none())
        );
    }
    assert!(peer.objects_to_destroy.is_empty());
    assert!(
        [parent, spawn, rider]
            .into_iter()
            .all(|id| peer.host_object(id).unwrap().is_alive())
    );
    peer.destroy_object(parent).unwrap();
    let queue: Vec<_> = peer
        .objects_to_destroy
        .iter()
        .map(|entry| entry.id)
        .collect();
    assert_eq!(
        queue,
        [parent, rider, spawn],
        "reversing declarations reverses sibling callbacks"
    );
}
