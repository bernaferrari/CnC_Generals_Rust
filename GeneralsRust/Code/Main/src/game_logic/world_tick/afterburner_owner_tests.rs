//! Actual immutable receiving Object template -> ordinary jet audio queue.
use super::*;

fn world(cue: Option<&str>) -> (GameLogic, ObjectId) {
    let sounds = cue
        .map(|cue| format!("UnitSpecificSounds\nAfterburner = {cue}\nEnd\n"))
        .unwrap_or_default();
    let source = format!(
        "Object ReceivingAfterburnerJet\nKindOf = AIRCRAFT\n{sounds}Body = ActiveBody BodyTag\nMaxHealth = 80\nInitialHealth = 80\nEnd\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "afterburner-owner.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let id = world
        .create_object(
            "ReceivingAfterburnerJet",
            Team::USA,
            glam::Vec3::new(3.0, 4.0, 5.0),
        )
        .unwrap();
    assert!(
        world
            .host_object(id)
            .unwrap()
            .thing()
            .template
            .receiving_client_template
            .is_some()
    );
    (world, id)
}
fn assert_event(world: &mut GameLogic, id: ObjectId, cue: &str, stop: bool) {
    let events = std::mem::take(&mut world.queued_audio_events);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, cue);
    assert_eq!(events[0].object_id, Some(id));
    assert_eq!(events[0].position, Some(glam::Vec3::new(3.0, 4.0, 5.0)));
    assert_eq!(events[0].stop, stop);
    assert_eq!(events[0].is_looping, !stop);
}
#[test]
fn afterburner_start_stop_use_receiving_actor_even_with_same_id_and_wrong_name_argument() {
    let (mut owner, id) = world(Some("OwnerAfterburner"));
    let (mut peer, peer_id) = world(Some("PeerAfterburner"));
    let (mut silent, silent_id) = world(None);
    assert_eq!((id, id), (peer_id, silent_id));
    let pos = glam::Vec3::new(3.0, 4.0, 5.0);
    for on in [true, false] {
        owner.queue_afterburner_per_unit_sound(id, "ForeignTemplateMustNotResolve", pos, on);
        assert_event(&mut owner, id, "OwnerAfterburner", !on);
        peer.queue_afterburner_per_unit_sound(peer_id, "ForeignTemplateMustNotResolve", pos, on);
        assert_event(&mut peer, peer_id, "PeerAfterburner", !on);
        silent.queue_afterburner_per_unit_sound(silent_id, "ReceivingAfterburnerJet", pos, on);
        assert!(
            silent.queued_audio_events.is_empty(),
            "missing receiving slot never acquires a peer cue or stop token"
        );
    }
    let (mut disabled, disabled_id) = world(Some("NoSound"));
    disabled.queue_afterburner_per_unit_sound(disabled_id, "ReceivingAfterburnerJet", pos, true);
    disabled.queue_afterburner_per_unit_sound(disabled_id, "ReceivingAfterburnerJet", pos, false);
    assert!(disabled.queued_audio_events.is_empty());
}
