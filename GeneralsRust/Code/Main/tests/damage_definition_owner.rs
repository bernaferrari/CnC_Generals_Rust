use generals_main::game_logic::{GameLogic, HostHealthEvents, Team};

fn world(sound: &str, effect: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let source = format!(
        "Object SharedVictim\n KindOf = VEHICLE\n SoundOnDamaged = {sound}\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
         Behavior = TransitionDamageFX DamageTag\n DamagedFXList1 = Loc X:0 Y:0 Z:0 FXList:{effect}\n End\nEnd\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("Object.ini".into(), source)])
        .unwrap();
    let id = world
        .create_object("SharedVictim", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}

#[test]
fn same_named_objects_keep_their_own_damage_transition_content() {
    let previous_services = gamelogic::system::engine_stores::active_services();
    let (mut first, first_id) = world("FirstDamage", "FirstFx");
    let (mut peer, peer_id) = world("PeerDamage", "PeerFx");
    assert_eq!(first_id, peer_id);

    let exercise = |world: &mut GameLogic, id, sound: &str, effect: &str| {
        let object = world.host_object_mut(id).unwrap();
        // Rebuild on demand to exercise the same lazy path used after restoration.
        object.transition_damage_fx = None;
        object.ensure_transition_damage_fx();
        let data = object.transition_damage_fx.as_ref().unwrap();
        assert_eq!(data.audio_for_state[1].as_deref(), Some(sound));
        assert_eq!(data.fx_for_state[1].as_deref(), Some(effect));
        object.take_damage(60.0, 1, &mut HostHealthEvents::default());
        assert!(object.health.current < object.health.maximum);
        assert!(
            object.pending_transition_damage_fx.iter().any(|event| {
                event.audio_name.as_deref() == Some(sound) && event.new_state == 1
            }),
            "damage transition events: {:?}, health: {:?}",
            object.pending_transition_damage_fx,
            object.health,
        );
    };
    exercise(&mut peer, peer_id, "PeerDamage", "PeerFx");
    peer.reset();
    assert!(std::sync::Arc::ptr_eq(
        &previous_services,
        &gamelogic::system::engine_stores::active_services(),
    ));
    exercise(&mut first, first_id, "FirstDamage", "FirstFx");
    drop(peer);
    let object = first.host_object_mut(first_id).unwrap();
    object.transition_damage_fx = None;
    object.ensure_transition_damage_fx();
    assert_eq!(
        object
            .transition_damage_fx
            .as_ref()
            .unwrap()
            .audio_for_state[1]
            .as_deref(),
        Some("FirstDamage")
    );
}
