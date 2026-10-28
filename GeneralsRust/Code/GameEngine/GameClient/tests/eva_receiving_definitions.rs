//! Each application's EVA speaker admits its own authored definitions.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::eva::{Eva, EvaFrameContext, EvaLocalPlayerContext, EvaMessage, EvaPlayback};

fn priority(eva: &Eva) -> u32 {
    eva.get_eva_check_info_by_name("LowPower")
        .unwrap()
        .priority()
}

#[test]
fn independent_eva_sources_and_failed_candidate_do_not_change_other_speakers() {
    let first_source = "EvaEvent LowPower\nPriority = 2\nTimeBetweenChecksMS = 1000\nExpirationTimeMS = 2000\nSideSounds\nSide = America\nSounds = FirstAnnouncement\nEnd\nEnd\n";
    let duplicate_source = "EvaEvent LowPower\nPriority = 99\nEnd\n";
    let second_source = "EvaEvent LowPower\nPriority = 7\nTimeBetweenChecksMS = 3000\nExpirationTimeMS = 4000\nEnd\n";
    let mut first = Eva::from_ordered_ini_sources([first_source, duplicate_source]).unwrap();
    let mut second = Eva::from_ordered_ini_sources([second_source]).unwrap();
    let first_info = first.get_eva_check_info_by_name("LOWPOWER").unwrap();
    assert_eq!(
        first_info.priority(),
        2,
        "a duplicate does not replace the first event"
    );
    assert_eq!(first_info.frames_between_checks(), 30);
    assert_eq!(first_info.frames_to_expire(), 60);
    let second_info = second.get_eva_check_info_by_name("LowPower").unwrap();
    assert_eq!(second_info.priority(), 7);
    assert_eq!(second_info.frames_between_checks(), 90);
    assert_eq!(second_info.frames_to_expire(), 120);
    first.set_should_play(EvaMessage::LowPower);
    second.set_should_play(EvaMessage::LowPower);
    first.reset();
    assert!(first.pending_messages().is_empty());
    assert_eq!(second.pending_messages(), vec![EvaMessage::LowPower]);
    assert!(Eva::from_ordered_ini_sources(["EvaEvent LowPower\nPriority = 5\n"]).is_err());
    assert_eq!(priority(&first), 2);
    assert_eq!(priority(&second), 7);
    drop(first);
    assert_eq!(priority(&second), 7);
}

#[test]
fn native_client_reset_does_not_reset_the_standalone_eva() {
    game_client_rust::eva::set_eva_enabled(false);
    let mut client = GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted)
        .expect("native hosted client");
    client.reset().expect("reset native client");
    assert!(!game_client_rust::eva::with_eva(|eva| eva.is_enabled()));
    game_client_rust::eva::set_eva_enabled(true);
}

#[derive(Default)]
struct CapturedSpeech(Vec<(String, Option<u32>)>);

impl EvaPlayback for CapturedSpeech {
    fn is_playing(&mut self, _handle: u32) -> bool {
        false
    }

    fn play_speech(&mut self, event: &gamelogic::common::audio::AudioEventRts) -> u32 {
        self.0.push((event.event_name.clone(), event.player_index));
        1000 + self.0.len() as u32
    }
}

#[test]
fn speakers_borrow_their_own_playback_and_sound_selection() {
    let source = "EvaEvent InsufficientFunds\nPriority = 2\nSideSounds\nSide = America\nSounds = FirstVoice SecondVoice\nEnd\nEnd\n";
    let mut first = Eva::from_ordered_ini_sources([source]).unwrap();
    let mut second = Eva::from_ordered_ini_sources([source]).unwrap();
    let context = |index| EvaFrameContext {
        frame: 2,
        local_player: Some(EvaLocalPlayerContext {
            side: "America".into(),
            index,
            sufficient_power: true,
        }),
    };
    let mut first_speech = CapturedSpeech::default();
    let mut second_speech = CapturedSpeech::default();
    first.set_should_play(EvaMessage::InsufficientFunds);
    second.set_should_play(EvaMessage::InsufficientFunds);
    first.update_with_playback(&context(2), &mut first_speech, &mut |lo, _| lo);
    assert_eq!(first_speech.0, [("FirstVoice".into(), Some(2))]);
    assert!(second_speech.0.is_empty());
    second.update_with_playback(&context(8), &mut second_speech, &mut |_, hi| hi);
    assert_eq!(second_speech.0, [("SecondVoice".into(), Some(8))]);
    assert_eq!(first_speech.0.len(), 1);
}
