//! Audio settings and Options preferences belong to their receiving instance.
use game_engine::common::{
    audio::game_audio::{AudioAffect, AudioManager, get_global_audio_manager},
    ini::{
        INI,
        ini_audio_settings::{AudioSettings, get_audio_settings},
    },
    user_preferences::UserPreferences,
};

fn admit(settings: &mut AudioSettings, prefs: &UserPreferences, root: &str) {
    let source = format!(
        r#"
AudioSettings
 AudioRoot = {root}
 Relative2DVolume = -20
 DefaultSoundVolume = 80
 Default3DSoundVolume = 70
 DefaultSpeechVolume = 60
 DefaultMusicVolume = 50
End
AudioEvent Unrelated
 Sounds = NotAdmitted
End
"#
    );
    INI::new()
        .with_inline_source(&source, |ini| {
            ini.parse_current_file_into_audio_settings(settings, prefs)
        })
        .unwrap();
}

#[test]
fn separate_preferences_and_definitions_never_publish_global_settings() {
    assert!(get_audio_settings().is_none());
    assert!(get_global_audio_manager().is_none());
    let mut left = AudioSettings::new();
    let mut right = AudioSettings::new();
    let left_options = UserPreferences::new();
    let mut right_options = UserPreferences::new();
    right_options.set_float("SFXVolume", 25.0);
    right_options.set_float("SFX3DVolume", 35.0);
    right_options.set_float("MusicVolume", 60.0);
    admit(&mut left, &left_options, "LeftRoot");
    admit(&mut right, &right_options, "RightRoot");
    assert_eq!(left.audio_root, "LeftRoot");
    assert_eq!(right.audio_root, "RightRoot");
    assert!((left.preferred_sound_volume - 0.64).abs() < 0.0001);
    assert!((left.preferred_3d_sound_volume - 0.70).abs() < 0.0001);
    assert!((left.preferred_music_volume - 0.50).abs() < 0.0001);
    assert!((right.preferred_sound_volume - 0.25).abs() < 0.0001);
    assert!((right.preferred_3d_sound_volume - 0.35).abs() < 0.0001);
    assert!((right.preferred_music_volume - 0.60).abs() < 0.0001);
    let mut left_manager = AudioManager::new();
    let mut right_manager = AudioManager::new();
    left_manager.apply_receiving_audio_settings(&left);
    right_manager.apply_receiving_audio_settings(&right);
    assert_eq!(left_manager.get_audio_settings().audio_root, "LeftRoot");
    assert_eq!(right_manager.get_audio_settings().audio_root, "RightRoot");
    assert!((left_manager.get_volume(AudioAffect::Sound) - 0.64).abs() < 0.0001);
    assert!((right_manager.get_volume(AudioAffect::Sound) - 0.25).abs() < 0.0001);
    assert!(get_global_audio_manager().is_none());
    assert!(get_audio_settings().is_none());
}
