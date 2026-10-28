use game_engine::common::system::build_assistant::CanMakeType;
use generals_main::game_logic::{
    GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn world_with_place_builder(script_disabled: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::Skirmish);
    world.clear_all_players();

    let mut owner = Player::new(0, Team::USA, "USA", true);
    owner.resources.supplies = 1000;
    world.add_player(owner);

    // This producer/target pair is in the legacy name table, which used to
    // approve the placement probe even when this receiving world had no
    // admitted CommandSet. The live query must fail closed on that absence.
    let mut builder = ThingTemplate::new("AmericaBarracks");
    builder.add_kind_of(KindOf::Structure);
    builder.add_kind_of(KindOf::FSBarracks);
    builder.add_kind_of(KindOf::Selectable);
    builder.set_health(1000.0);
    world.templates.insert(builder.name.clone(), builder);

    let mut target = ThingTemplate::new("AmericaInfantryRanger");
    target.add_kind_of(KindOf::Infantry);
    target.set_health(100.0);
    target.set_cost(150, 0);
    world.templates.insert(target.name.clone(), target);

    let builder_id = world
        .create_object("AmericaBarracks", Team::USA, Vec3::ZERO)
        .expect("receiving-world producer should be admitted");
    world
        .host_object_mut(builder_id)
        .expect("dozer remains in receiving world")
        .set_status_disabled_script_disabled(script_disabled);
    (world, builder_id)
}

#[test]
fn same_id_placement_probe_fails_closed_without_receiving_command_set() {
    let (owner, owner_id) = world_with_place_builder(true);
    let (peer, peer_id) = world_with_place_builder(false);
    assert_eq!(
        owner_id, peer_id,
        "sessions should reuse the same object ID"
    );

    // Helper-only regression: the CnCGameEngine placement callback is private
    // to its native UI owner, so this exercises its public receiving-GameLogic
    // probe. C++ checks script-disabled first; absent an admitted command set,
    // isPossibleToMakeUnit is NoPrereq.
    assert_eq!(
        owner.can_make_unit_for_place(owner_id, "AmericaInfantryRanger"),
        CanMakeType::FactoryIsDisabled
    );
    assert_eq!(
        peer.can_make_unit_for_place(peer_id, "AmericaInfantryRanger"),
        CanMakeType::NoPrereq
    );
}
