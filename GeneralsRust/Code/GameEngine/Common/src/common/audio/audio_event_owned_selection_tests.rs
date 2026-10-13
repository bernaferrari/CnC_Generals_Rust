//! Actual event filename/play state with supplied configuration, no audio globals.
use super::*;

struct Selection;
impl AudioEventSelection for Selection {
    fn index(&mut self, _: bool, _: usize) -> usize {
        0
    }
    fn real(&mut self, low: f32, _: f32) -> f32 {
        low
    }
}

#[test]
fn supplied_event_paths_and_play_info_stay_with_each_receiving_operation() {
    let info = Arc::new(AudioEventInfo {
        audio_name: "OwnedFxSound".into(),
        sounds: vec!["impact".into()],
        attack_sounds: vec!["attack".into()],
        decay_sounds: vec!["decay".into()],
        pitch_shift_min: 0.8,
        pitch_shift_max: 0.8,
        volume_shift: -0.25,
        delay_min: 12.0,
        delay_max: 12.0,
        loop_count: 4,
        ..Default::default()
    });
    let mut first = AudioEventRts::with_event_name("OwnedFxSound");
    let mut peer = AudioEventRts::with_event_name("OwnedFxSound");
    first.set_audio_event_info(info.clone());
    peer.set_audio_event_info(info);
    let first_paths =
        AudioPathSettings::from_settings(&crate::common::audio::game_audio::AudioSettings {
            audio_root: "FirstAudio".into(),
            ..Default::default()
        });
    let peer_paths =
        AudioPathSettings::from_settings(&crate::common::audio::game_audio::AudioSettings {
            audio_root: "PeerAudio".into(),
            ..Default::default()
        });
    first.generate_filename_with_selection(&first_paths, &mut Selection);
    first.generate_play_info_with_selection(&first_paths, &mut Selection);
    peer.generate_filename_with_selection(&peer_paths, &mut Selection);
    peer.generate_play_info_with_selection(&peer_paths, &mut Selection);
    assert!(first.get_filename().starts_with("FirstAudio"));
    assert!(peer.get_filename().starts_with("PeerAudio"));
    assert!(first.get_attack_filename().ends_with("attack.wav"));
    assert!(first.get_decay_filename().ends_with("decay.wav"));
    assert_eq!(first.get_pitch_shift(), 0.8);
    assert_eq!(first.get_volume_shift(), 0.75);
    assert_eq!(first.get_delay(), 12.0);
    assert_eq!(first.get_loop_count(), 4);
    assert_eq!(first.portion_to_play_next, PortionToPlay::Attack);
}
