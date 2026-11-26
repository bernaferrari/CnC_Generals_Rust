use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use generals_main::presentation_frame::PresentationFrame;
use glam::Vec3;

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("FrozenBirthProbe");
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("FrozenBirthProbe", Team::USA, Vec3::ZERO)
        .unwrap();
    (world, id)
}

#[test]
fn victory_frame_actors_preserve_their_owning_world_identity() {
    let (mut owner, id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(id, peer_id);
    let first = PresentationFrame::build_with_victory(&mut owner, 0);
    let second = PresentationFrame::build_with_victory(&mut peer, 0);
    let actor = first.objects.iter().find(|object| object.id == id).unwrap();
    let peer_actor = second
        .objects
        .iter()
        .find(|object| object.id == peer_id)
        .unwrap();
    assert_ne!(
        actor, peer_actor,
        "equal-ID actors from different worlds retain different drawable identities"
    );
}
