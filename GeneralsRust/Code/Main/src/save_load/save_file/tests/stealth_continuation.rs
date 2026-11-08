use super::*;

#[test]
fn stealth_opacity_continues_from_real_archive_in_interleaved_worlds() {
    fn receiving(seed: u32) -> GameLogic {
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "archive-sneaker.ini".into(),
                "Object ArchiveSneaker\n\
                 KindOf = INFANTRY SELECTABLE\n\
                 Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                 Behavior = LifetimeUpdate LifetimeTag\nMinLifetime = 60000\nMaxLifetime = 90000\nEnd\n\
                 Behavior = StealthUpdate StealthTag\nInnateStealth = Yes\nStealthDelay = 0\nEnd\n\
                 Behavior = DeletionUpdate DeletionTag\nMinLifetime = 120000\nMaxLifetime = 150000\nEnd\n\
                 End\n".into(),
            )]).unwrap(),
        ).unwrap();
        world.add_player(Player::new(0, Team::GLA, "GLA", true));
        world.seed_fx_random(seed);
        world
    }

    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let mut source = receiving(0x1234);
    let mut peer = receiving(0x9876);
    // Lifetime and Deletion each consume one logic draw; Stealth consumes
    // one client draw. These separate streams do not establish cross-module
    // constructor ordering.
    let mut expected_logic = source.logic_random.snapshot();
    expected_logic.next_u32();
    expected_logic.next_u32();
    let peer_logic_before = peer.logic_random.seed_words();
    let mut source_random = source.session_view.client_random.clone();
    let mut peer_random = peer.session_view.client_random.clone();
    let source_first_phase = source_random.next_real(0.0, std::f32::consts::PI);
    let peer_first_phase = peer_random.next_real(0.0, std::f32::consts::PI);
    let peer_before_source_creation = peer.session_view.client_random.seed_words();
    let id = source.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    assert_eq!(peer.session_view.client_random.seed_words(), peer_before_source_creation);
    assert_eq!(source.logic_random.seed_words(), expected_logic.seed_words());
    assert_eq!(peer.logic_random.seed_words(), peer_logic_before);
    let peer_id = peer.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    source.host_object_mut(id).unwrap().owner_player_id = Some(0);
    peer.host_object_mut(peer_id).unwrap().owner_player_id = Some(0);
    assert_eq!(id, peer_id);
    assert_eq!(source.host_object(id).unwrap().camo_opacity_pulse_phase, source_first_phase);
    assert_eq!(peer.host_object(peer_id).unwrap().camo_opacity_pulse_phase, peer_first_phase);
    assert_eq!(source.session_view.client_random.seed_words(), source_random.seed_words());
    assert_eq!(peer.session_view.client_random.seed_words(), peer_random.seed_words());
    let initial_phase = source_first_phase;
    for frame in 1..=5 {
        source.frame = frame;
        source.update_stealth_and_detection();
        peer.frame = frame;
        peer.update_stealth_and_detection();
    }
    assert_ne!(source.host_object(id).unwrap().camo_opacity_pulse_phase, initial_phase);
    assert_eq!(source.session_view.client_random.seed_words(), source_random.seed_words());
    assert_eq!(peer.session_view.client_random.seed_words(), peer_random.seed_words());
    // Xfer retains the rate as well as the phase, including non-default rates.
    source.host_object_mut(id).unwrap().camo_opacity_pulse_phase_rate = 0.37;
    saves.save_game("stealth", &source, &fixture_save_info()).unwrap();
    let saved = saves.load_game_snapshot("stealth").unwrap().snapshot;
    let mut restored = receiving(0x4567);
    let mut expected_random = restored.session_view.client_random.clone();
    expected_random.next_real(0.0, std::f32::consts::PI);
    saves.restore_game_snapshot(&saved, &mut restored).unwrap();
    assert_eq!(restored.session_view.client_random.seed_words(), expected_random.seed_words());
    assert_eq!(source.session_view.client_random.seed_words(), source_random.seed_words());
    assert_eq!(peer.session_view.client_random.seed_words(), peer_random.seed_words());
    assert_eq!(restored.host_object(id).unwrap().camo_opacity_pulse_phase_rate, 0.37);
    assert_eq!(peer.host_object(peer_id).unwrap().camo_opacity_pulse_phase_rate, 0.2);
    assert_eq!(
        restored.host_object(id).unwrap().camo_opacity_pulse_phase,
        source.host_object(id).unwrap().camo_opacity_pulse_phase
    );
    for frame in 6..=10 {
        let phase_before = source.host_object(id).unwrap().camo_opacity_pulse_phase;
        peer.frame = frame;
        peer.update_stealth_and_detection();
        source.frame = frame;
        source.update_stealth_and_detection();
        restored.frame = frame;
        restored.update_stealth_and_detection();
        assert_eq!(
            source.host_object(id).unwrap().camo_opacity_pulse_phase,
            phase_before + 0.37
        );
        let original = source.host_object(id).unwrap();
        let loaded = restored.host_object(id).unwrap();
        assert_eq!(loaded.camo_opacity_pulse_phase, original.camo_opacity_pulse_phase);
        assert_eq!(loaded.camo_friendly_opacity, original.camo_friendly_opacity);
        let rendered = crate::presentation_frame::PresentationFrame::build_from_logic(&mut restored, 0)
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == id)
            .expect("restored friendly stealth is renderable");
        assert!((rendered.presentation_opacity - original.camo_friendly_opacity).abs() < 1e-5);
        let phase = restored.host_object(id).unwrap().camo_opacity_pulse_phase;
        let random = restored.session_view.client_random.seed_words();
        let _ = crate::presentation_frame::PresentationFrame::build_from_logic(&mut restored, 0);
        assert_eq!(restored.host_object(id).unwrap().camo_opacity_pulse_phase, phase);
        assert_eq!(restored.session_view.client_random.seed_words(), random);
    }
    assert_eq!(source.session_view.client_random.seed_words(), source_random.seed_words());
    assert_eq!(peer.session_view.client_random.seed_words(), peer_random.seed_words());
    assert_eq!(restored.session_view.client_random.seed_words(), expected_random.seed_words());

    // Loading overlays the saved phase, but the next constructor must continue
    // the receiving client's stream, not the saved world's cosmetic stream.
    let source_before_restored_creation = source.session_view.client_random.seed_words();
    let peer_before_restored_creation = peer.session_view.client_random.seed_words();
    let restored_next = expected_random.next_real(0.0, std::f32::consts::PI);
    let source_next = source_random.next_real(0.0, std::f32::consts::PI);
    let peer_next = peer_random.next_real(0.0, std::f32::consts::PI);
    let restored_id = restored.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    assert_eq!(source.session_view.client_random.seed_words(), source_before_restored_creation);
    assert_eq!(peer.session_view.client_random.seed_words(), peer_before_restored_creation);
    let peer_second = peer.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    let source_second = source.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    assert_eq!(restored.host_object(restored_id).unwrap().camo_opacity_pulse_phase, restored_next);
    assert_eq!(peer.host_object(peer_second).unwrap().camo_opacity_pulse_phase, peer_next);
    assert_eq!(source.host_object(source_second).unwrap().camo_opacity_pulse_phase, source_next);
    assert_eq!(restored.session_view.client_random.seed_words(), expected_random.seed_words());
    assert_eq!(peer.session_view.client_random.seed_words(), peer_random.seed_words());
    assert_eq!(source.session_view.client_random.seed_words(), source_random.seed_words());
    assert_ne!(
        peer.host_object(peer_id).unwrap().camo_opacity_pulse_phase,
        restored.host_object(id).unwrap().camo_opacity_pulse_phase
    );
}
