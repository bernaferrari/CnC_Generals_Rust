use super::*;

fn world_with_unit(id: ObjectId, origin: glam::Vec3) -> GameLogic {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("OwnedWander");
    template.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(template, id, Team::USA);
    unit.set_position(origin);
    world.objects.insert(id, unit);
    world
}

#[test]
fn same_id_wander_commands_and_missing_foreign_tick_preserve_the_owner() {
    let id = ObjectId(0xC041);
    let origin_a = glam::Vec3::new(10.0, 0.0, 10.0);
    let origin_b = glam::Vec3::new(900.0, 0.0, 900.0);
    let mut a = world_with_unit(id, origin_a);
    a.test_host_wander_in_place(id, origin_a);
    let a_hop = a.test_wander_in_place_hop(id).expect("A admitted wander");
    let mut b = world_with_unit(id, origin_b);
    b.test_host_wander_in_place(id, origin_b);
    assert_eq!(
        a.test_wander_in_place_hop(id),
        Some(a_hop),
        "B's command cannot replace A's hop"
    );
    assert_ne!(b.test_wander_in_place_hop(id), Some(a_hop));
    let mut empty = GameLogic::new();
    empty.test_tick_host_wander_in_place();
    assert_eq!(
        a.test_wander_in_place_hop(id),
        Some(a_hop),
        "a world without this ID cannot erase A's state"
    );
    b.test_tick_host_wander_in_place();
    assert_eq!(a.test_wander_in_place_hop(id), Some(a_hop));
}

#[test]
fn wander_runtime_ends_with_the_object_and_new_ids_start_inert() {
    let id = ObjectId(0xC042);
    let origin = glam::Vec3::new(10.0, 0.0, 10.0);
    let mut world = world_with_unit(id, origin);
    world.test_host_wander_in_place(id, origin);
    let mut copied = world.host_object(id).unwrap().clone();
    let live_state = world
        .host_object(id)
        .unwrap()
        .unit_ai_runtime
        .wander_in_place();
    assert_eq!(copied.unit_ai_runtime.wander_in_place(), live_state);
    copied.unit_ai_runtime.set_wander(None);
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .wander_in_place(),
        live_state,
        "query clones copy state values without sharing a machine"
    );
    world.objects.remove(&id);
    world.test_host_wander_in_place(id, origin);
    world.test_host_wander_issue_path(id, &[glam::Vec3::new(50.0, 0.0, 50.0)]);
    assert!(!world.test_wander_in_place_active(id));
    assert!(!world.test_wander_path_active(id));
    let replacement = world_with_unit(id, origin).objects.remove(&id).unwrap();
    world.objects.insert(id, replacement);
    assert!(
        !world.test_wander_in_place_active(id),
        "ID reuse cannot inherit a destroyed machine"
    );
    world.test_host_wander_issue_path(id, &[glam::Vec3::new(60.0, 0.0, 60.0)]);
    assert!(world.test_wander_path_active(id));
    world.start_new_game(GameMode::Skirmish);
    assert!(
        !world.test_wander_path_active(id),
        "reset drops the old object's machine"
    );
}
