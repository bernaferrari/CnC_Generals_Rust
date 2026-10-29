use generals_main::game_logic::{GameLogic, Team, ThingTemplate};
use generals_main::gameworld_shadow::GameWorldShadow;
use glam::Vec3;

fn health_world(initial_health: f32) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("ShadowHealthProjectionProbe");
    template.set_health(100.0);
    template.add_kind_of(generals_main::game_logic::KindOf::Selectable);
    logic
        .templates
        .insert("ShadowHealthProjectionProbe".into(), template);
    let id = logic
        .create_object("ShadowHealthProjectionProbe", Team::USA, Vec3::ZERO)
        .expect("fixture Object must be created in its receiving GameLogic");
    logic
        .host_object_mut(id)
        .expect("receiving Object remains registered")
        .health
        .current = initial_health;
    (logic, id)
}

#[test]
fn health_projection_follows_each_receiving_world() {
    let (mut first, first_id) = health_world(80.0);
    let (mut peer, peer_id) = health_world(60.0);
    assert_eq!(first_id, peer_id, "Object IDs are session local");
    let mut first_shadow = GameWorldShadow::new(16);
    let mut peer_shadow = GameWorldShadow::new(16);

    first_shadow.sync_from_host(&first);
    peer_shadow.sync_from_host(&peer);
    assert_eq!(first_shadow.health_parity(&first), (true, 1));
    assert_eq!(peer_shadow.health_parity(&peer), (true, 1));

    first
        .host_object_mut(first_id)
        .expect("first Object remains registered")
        .health
        .current = 35.0;
    peer.host_object_mut(peer_id)
        .expect("peer Object remains registered")
        .health
        .current = 45.0;
    first_shadow.sync_from_host(&first);

    let first_entity = first_shadow
        .entity_for_host(first_id)
        .expect("first projection keeps its host mapping");
    let peer_entity = peer_shadow
        .entity_for_host(peer_id)
        .expect("peer projection keeps its host mapping");
    assert_eq!(
        first_shadow.world().entity(first_entity).unwrap().health,
        35.0,
        "sync copies canonical host HP into the observer"
    );
    assert_eq!(
        peer_shadow.world().entity(peer_entity).unwrap().health,
        60.0,
        "syncing the first world leaves its same-ID peer untouched"
    );
    assert_eq!(first_shadow.health_parity(&first), (true, 1));
    assert_eq!(peer_shadow.health_parity(&peer), (false, 1));
    peer_shadow.sync_from_host(&peer);
    assert_eq!(peer_shadow.health_parity(&peer), (true, 1));
}
