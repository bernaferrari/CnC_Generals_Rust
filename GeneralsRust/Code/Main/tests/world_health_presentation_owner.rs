use generals_main::game_logic::{GameLogic, ObjectId, Team, ThingTemplate};
use generals_main::presentation_frame::PresentationFrame;
use glam::Vec3;

fn health_world(initial_health: f32) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("HealthPresentationProbe");
    template.set_health(100.0);
    template.add_kind_of(generals_main::game_logic::KindOf::Selectable);
    logic
        .templates
        .insert("HealthPresentationProbe".into(), template);
    let id = logic
        .create_object("HealthPresentationProbe", Team::USA, Vec3::ZERO)
        .expect("fixture Object must be created in its receiving GameLogic");
    logic
        .host_object_mut(id)
        .expect("receiving Object remains registered")
        .health
        .current = initial_health;
    (logic, id)
}

fn visible_health(frame: &PresentationFrame, id: ObjectId) -> f32 {
    frame
        .objects
        .iter()
        .find(|object| object.id == id)
        .expect("receiving object has a presentation row")
        .health_current
}

#[test]
fn health_presentation_follows_each_receiving_world() {
    let (mut first, first_id) = health_world(80.0);
    let (mut peer, peer_id) = health_world(60.0);
    assert_eq!(first_id, peer_id, "Object IDs are session local");
    let first_before = PresentationFrame::try_build_from_logic(&mut first, 0).unwrap();
    let peer_before = PresentationFrame::try_build_from_logic(&mut peer, 0).unwrap();
    assert_eq!(visible_health(&first_before, first_id), 80.0);
    assert_eq!(visible_health(&peer_before, peer_id), 60.0);

    first.host_object_mut(first_id).unwrap().health.current = 35.0;
    peer.host_object_mut(peer_id).unwrap().health.current = 45.0;
    let first_frame = PresentationFrame::try_build_from_logic(&mut first, 0).unwrap();
    assert_eq!(visible_health(&first_frame, first_id), 35.0);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 45.0);
    assert_eq!(visible_health(&first_before, first_id), 80.0);
    assert_eq!(visible_health(&peer_before, peer_id), 60.0);

    let peer_frame = PresentationFrame::try_build_from_logic(&mut peer, 0).unwrap();
    assert_eq!(visible_health(&peer_frame, peer_id), 45.0);
    assert_eq!(visible_health(&first_frame, first_id), 35.0);
}
