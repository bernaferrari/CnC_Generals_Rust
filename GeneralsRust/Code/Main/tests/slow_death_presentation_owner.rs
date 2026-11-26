use generals_main::game_logic::host_slow_death::HostSlowDeathIni;
use generals_main::game_logic::{
    GameLogic, KindOf, ObjectAudio, ObjectId, SessionAudio, Team, ThingTemplate,
};
use generals_main::presentation_frame::PresentationFrame;
use glam::Vec3;

fn world(start_timer: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("OwnedSlowDeath");
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    world.templates.insert("OwnedSlowDeath".into(), template);
    let id = world
        .create_object("OwnedSlowDeath", Team::USA, Vec3::new(0.0, 10.0, 0.0))
        .unwrap();
    if start_timer {
        let frame = world.getFrame();
        let mut audio = SessionAudio::default();
        let ini = HostSlowDeathIni {
            destruction_delay_ms: 300,
            ..Default::default()
        };
        assert!(
            world
                .host_object_mut(id)
                .unwrap()
                .begin_slow_death_from_ini(frame, &ini, &mut ObjectAudio::queue(&mut audio))
        );
    }
    (world, id)
}

#[test]
fn slow_death_stays_with_receiving_world_during_peer_publication() {
    let (mut owner, id) = world(true);
    let (mut peer, peer_id) = world(false);
    assert_eq!(id, peer_id);

    let owner_frame = PresentationFrame::build_from_logic(&mut owner, 0);
    assert!(owner_frame.objects.iter().any(|object| object.id == id));
    for _ in 0..3 {
        peer.update();
        let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
        assert!(peer_frame.objects.iter().any(|object| object.id == peer_id));
    }
    assert_eq!(owner.getFrame(), 0);

    assert!(peer.host_object(peer_id).is_some());
    let due = owner
        .host_object(id)
        .unwrap()
        .slow_death
        .as_ref()
        .unwrap()
        .destroy_at_frame;
    assert_eq!(due, 9, "300 ms occupies nine 30 Hz logic frames");
    while owner.getFrame() <= due {
        let frame = owner.getFrame();
        owner.update();
        assert_eq!(owner.getFrame(), frame + 1);
        assert_eq!(
            owner.host_object(id).is_some(),
            frame < due,
            "the receiving owner alone retires the object at its deadline"
        );
    }
    assert!(
        owner.host_object(id).is_none(),
        "the owner's timer must finish"
    );
    assert!(
        peer.host_object(peer_id).is_some(),
        "a same-ID peer stays alive"
    );
    peer.update();
    let retired = PresentationFrame::build_from_logic(&mut owner, 0);
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert!(retired.objects.iter().all(|object| object.id != id));
    assert!(peer.host_object(peer_id).is_some());
    assert!(peer_frame.objects.iter().any(|object| object.id == peer_id));
    assert!(owner_frame.objects.iter().any(|object| object.id == id));
}
