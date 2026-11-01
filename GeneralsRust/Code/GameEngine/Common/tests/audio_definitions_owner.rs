//! Real INI blocks must target only the receiving audio manager.
use game_engine::common::{
    audio::game_audio::{AudioManager, get_global_audio_manager},
    ini::INI,
};

fn admit(manager: &mut AudioManager, source: &str) {
    INI::new()
        .with_inline_source(source, |ini| ini.parse_current_file_into_audio(manager))
        .unwrap();
}

#[test]
fn audio_defaults_overrides_and_other_blocks_stay_in_receiving_manager() {
    assert!(get_global_audio_manager().is_none());
    let mut left = AudioManager::new();
    let mut right = AudioManager::new();
    admit(
        &mut left,
        r#"
AudioEvent DefaultSoundEffect
 Volume = 60
 Sounds = LeftDefault
End
AudioEvent Shared
 Sounds = LeftOne LeftTwo
End
MusicTrack DefaultMusicTrack
 Volume = 40
End
MusicTrack TrackOne
 Sounds = ThemeOne
End
MusicTrack TrackTwo
 Sounds = ThemeTwo
End
DialogEvent DefaultDialog
 Volume = 30
End
DialogEvent Speak
 Sounds = LeftVoice
End
Weather
 SnowEnabled = Yes
End
"#,
    );
    admit(
        &mut right,
        r#"
AudioEvent DefaultSoundEffect
 Volume = 25
 Sounds = RightDefault
End
AudioEvent Shared
 Sounds = RightOne
End
"#,
    );
    let l = left.find_audio_event_info("Shared").unwrap();
    let r = right.find_audio_event_info("Shared").unwrap();
    assert!((l.volume - 0.6).abs() < 0.0001);
    assert!((r.volume - 0.25).abs() < 0.0001);
    assert_eq!(l.sounds, ["LeftOne", "LeftTwo"]);
    assert_eq!(r.sounds, ["RightOne"]);
    assert!((left.find_audio_event_info("Speak").unwrap().volume - 0.3).abs() < 0.0001);
    assert!(right.find_audio_event_info("Speak").is_none());
    assert_eq!(
        left.music_track_names(),
        ["DefaultMusicTrack", "TrackOne", "TrackTwo"]
    );
    assert!(right.music_track_names().is_empty());
    assert!((left.find_audio_event_info("TrackTwo").unwrap().volume - 0.4).abs() < 0.0001);

    admit(
        &mut left,
        "AudioEvent Shared\n Volume = 90\n Sounds = Replacement\nEnd\n",
    );
    let updated = left.find_audio_event_info("Shared").unwrap();
    assert!((updated.volume - 0.9).abs() < 0.0001);
    assert_eq!(updated.sounds, ["Replacement"]);
    assert_eq!(
        right.find_audio_event_info("Shared").unwrap().sounds,
        ["RightOne"]
    );
    assert!(get_global_audio_manager().is_none());
}
