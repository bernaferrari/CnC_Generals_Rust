use generals_main::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use glam::Vec3;

fn world() -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("ChinaNuclearMissileLauncher");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("ChinaNuclearMissileLauncher", Team::China, Vec3::ZERO)
        .unwrap();
    (world, id)
}

#[test]
fn launcher_initialization_waits_for_construction_and_stays_world_owned() {
    let (mut owner, id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(id, peer_id);
    owner.host_object_mut(id).unwrap().status.under_construction = true;
    owner.update();
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .missile_launcher_building
            .is_none()
    );
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .missile_launcher_building
            .is_none()
    );
    owner.host_object_mut(id).unwrap().status.under_construction = false;
    owner.update();
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .missile_launcher_building
            .is_some()
    );
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .missile_launcher_building
            .is_none()
    );
    peer.update();
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .missile_launcher_building
            .is_some()
    );
}
