//! MiscAudio definitions and projected UI events stay with their recipient.
use game_engine::common::{
    audio::game_audio::{AudioManager, get_global_audio_manager},
    ini::{
        INI,
        ini_misc_audio::{MiscAudio, get_misc_audio},
    },
};

fn admit(audio: &mut MiscAudio, source: &str) {
    INI::new()
        .with_inline_source(source, |ini| ini.parse_current_file_into_misc_audio(audio))
        .unwrap();
}

#[test]
fn repeated_blocks_override_only_receiving_world_and_do_not_publish() {
    assert!(get_misc_audio().is_none());
    assert!(get_global_audio_manager().is_none());
    let mut left = MiscAudio::new();
    let mut right = MiscAudio::new();
    admit(
        &mut left,
        r#"
MiscAudio
 RadarNotifyUnitUnderAttackSound = LeftAttack
 RadarNotifyInfiltrationSound = LeftInfiltration
End
AudioEvent Unrelated
 Sounds = NotAdmitted
End
MiscAudio
 RadarNotifyUnitUnderAttackSound = LeftReplacement
End
"#,
    );
    admit(
        &mut right,
        "MiscAudio\n RadarNotifyUnitUnderAttackSound = RightAttack\nEnd\n",
    );
    let mut left_manager = AudioManager::new();
    let mut right_manager = AudioManager::new();
    left_manager.apply_receiving_misc_audio(&left);
    right_manager.apply_receiving_misc_audio(&right);
    let event = |manager: &AudioManager, key: &str| {
        manager
            .get_misc_audio()
            .ui_sounds
            .get(key)
            .map(|event| event.get_event_name().to_owned())
    };
    assert_eq!(
        event(&left_manager, "RadarNotifyUnitUnderAttackSound").as_deref(),
        Some("LeftReplacement")
    );
    assert_eq!(
        event(&left_manager, "RadarNotifyInfiltrationSound").as_deref(),
        Some("LeftInfiltration")
    );
    assert_eq!(
        event(&right_manager, "RadarNotifyUnitUnderAttackSound").as_deref(),
        Some("RightAttack")
    );
    assert_eq!(event(&right_manager, "RadarNotifyInfiltrationSound"), None);
    assert!(get_misc_audio().is_none());
    assert!(get_global_audio_manager().is_none());
}
