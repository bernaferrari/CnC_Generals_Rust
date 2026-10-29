use generals_main::game_logic::host_fire_spread::HostFlammableState;
use generals_main::game_logic::{GameLogic, GameMode, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn missing_receiving_ember_ocl_does_not_synthesize_a_gameplay_object() {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);

    let mut tree = ThingTemplate::new("DogwoodTreeFireSpreadOclOwner");
    tree.set_health(100.0);
    world.templates.insert(tree.name.clone(), tree);
    let tree = world
        .create_object("DogwoodTreeFireSpreadOclOwner", Team::Neutral, Vec3::ZERO)
        .expect("create source tree");

    let fire = world
        .host_object_mut(tree)
        .and_then(|object| object.fire_spread.as_mut())
        .expect("tree admits FlammableUpdate and FireSpreadUpdate");
    fire.state = HostFlammableState::Aflame;
    fire.body_aflame = true;
    fire.aflame_end_frame = u32::MAX;
    fire.next_spread_frame = 0;
    fire.spread_try_range = 0.0;

    // The public fixed-step path reaches the real FireSpreadUpdate callback.
    // With no admitted OCL, C++ ObjectCreationList::create has no create
    // nuggets to run; Main must not invent a generic BurningEmbers actor.
    world.update_with_dt(1.0 / 30.0);

    assert_eq!(world.host_objects().registered_len(), 1);
    let source = world.host_object(tree).expect("source stays live");
    assert!(source.fire_spread.as_ref().unwrap().next_spread_frame > 0);
}
