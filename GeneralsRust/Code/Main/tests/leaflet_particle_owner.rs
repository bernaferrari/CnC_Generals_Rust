use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world(particle: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "leaflet-owner.ini".into(),
            format!(
                "Object LeafletContainer\n KindOf = PROJECTILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = LeafletDropBehavior LeafletTag\n LeafletFXParticleSystem = {particle}\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let id = world
        .create_object_for_player("LeafletContainer", 0, Vec3::new(10.0, 20.0, 10.0))
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.leaflet_container = true;
    object.movement.velocity = Vec3::ZERO;
    (world, id)
}

#[test]
fn leaflet_particles_use_each_containers_admitted_definition() {
    let (mut first, first_id) = world("FirstLeafletFX");
    let (mut second, second_id) = world("SecondLeafletFX");
    let (mut silent, silent_id) = world("NONE");
    assert_eq!((first_id, second_id), (second_id, silent_id));

    second.update_leaflet_b52_flights();
    silent.update_leaflet_b52_flights();
    first.update_leaflet_b52_flights();

    let names = |world: &GameLogic| {
        world
            .combat_particles()
            .active_systems()
            .map(|particle| particle.template_name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&first), ["FirstLeafletFX"]);
    assert_eq!(names(&second), ["SecondLeafletFX"]);
    assert!(names(&silent).is_empty(), "NONE admits no particle");

    first.update_leaflet_b52_flights();
    assert_eq!(names(&first), ["FirstLeafletFX"], "FX starts only once");
    assert_eq!(names(&second), ["SecondLeafletFX"]);
}
