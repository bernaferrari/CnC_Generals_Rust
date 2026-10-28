use generals_main::game_logic::{
    GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate,
    special_power_strikes::ArtilleryBarrageScienceTier,
};

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::China, "Owner", true));
    let mut template = ThingTemplate::new("BarrageSource");
    template.add_kind_of(KindOf::Structure).set_health(1000.0);
    world.templates.insert(template.name.clone(), template);
    let source = world
        .create_object_for_player("BarrageSource", 1, glam::Vec3::ZERO)
        .unwrap();
    (world, source)
}

fn launch(world: &mut GameLogic, source: ObjectId) -> Vec<u32> {
    world
        .spawn_artillery_barrage_flight(
            source,
            glam::Vec3::new(200.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level1,
        )
        .unwrap();
    world
        .host_objects()
        .registered_values()
        .filter_map(|object| {
            object
                .artillery_barrage_transport
                .as_ref()
                .map(|transport| transport.delay_until_frame)
        })
        .collect()
}

#[test]
fn another_worlds_barrage_does_not_change_the_receiving_schedule() {
    let (mut alone, alone_source) = world();
    let (mut receiving, receiving_source) = world();
    let (mut peer, peer_source) = world();
    assert_eq!(alone_source, receiving_source);
    assert_eq!(alone_source, peer_source);

    let expected = launch(&mut alone, alone_source);
    let peer_schedule = launch(&mut peer, peer_source);
    let actual = launch(&mut receiving, receiving_source);

    assert_eq!(expected.len(), 12);
    assert_eq!(actual, expected);
    assert_eq!(peer_schedule, expected);
}
