use super::*;
use crate::game_logic::object::StealthUpdateData;

#[test]
fn stealth_opacity_continues_from_real_archive_in_interleaved_worlds() {
    fn receiving(seed: u32) -> GameLogic {
        let mut world = GameLogic::new();
        let mut template = ThingTemplate::new("ArchiveSneaker");
        template.set_health(100.0);
        template.stealth_update = Some(StealthUpdateData {
            innate_stealth: true,
            ..Default::default()
        });
        world.templates.insert("ArchiveSneaker".into(), template);
        world.add_player(Player::new(0, Team::GLA, "GLA", true));
        world.seed_fx_random(seed);
        world
    }

    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let mut source = receiving(0x1234);
    let mut peer = receiving(0x9876);
    let id = source.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    let peer_id = peer.create_object("ArchiveSneaker", Team::GLA, Vec3::ZERO).unwrap();
    source.host_object_mut(id).unwrap().owner_player_id = Some(0);
    peer.host_object_mut(peer_id).unwrap().owner_player_id = Some(0);
    assert_eq!(id, peer_id);
    let initial_phase = source.host_object(id).unwrap().camo_opacity_pulse_phase;
    for frame in 1..=5 {
        source.frame = frame;
        source.update_stealth_and_detection();
    }
    assert_ne!(source.host_object(id).unwrap().camo_opacity_pulse_phase, initial_phase);
    saves.save_game("stealth", &source, &fixture_save_info()).unwrap();
    let saved = saves.load_game_snapshot("stealth").unwrap().snapshot;
    let mut restored = receiving(0x4567);
    let mut expected_random = restored.session_view.client_random.clone();
    expected_random.next_real(0.0, std::f32::consts::PI);
    saves.restore_game_snapshot(&saved, &mut restored).unwrap();
    assert_eq!(restored.session_view.client_random.seed_words(), expected_random.seed_words());
    assert_eq!(
        restored.host_object(id).unwrap().camo_opacity_pulse_phase,
        source.host_object(id).unwrap().camo_opacity_pulse_phase
    );
    for frame in 6..=10 {
        peer.frame = frame;
        peer.update_stealth_and_detection();
        source.frame = frame;
        source.update_stealth_and_detection();
        restored.frame = frame;
        restored.update_stealth_and_detection();
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
    }
    assert_ne!(
        peer.host_object(peer_id).unwrap().camo_opacity_pulse_phase,
        restored.host_object(id).unwrap().camo_opacity_pulse_phase
    );
}
