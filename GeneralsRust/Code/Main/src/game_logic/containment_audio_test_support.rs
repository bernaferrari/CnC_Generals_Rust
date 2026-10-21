//! Real receiving definitions and real pending audio requests; no receipt log.
use super::*;
use game_engine::common::audio::{AudioEventInfo, AudioManager, AudioType, ST_GLOBAL};

pub(crate) fn admit(world: &mut GameLogic, source: &str) {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "containment-audio-owner.ini".to_owned(),
        source.to_owned(),
    )])
    .unwrap();
    world.admit_receiving_module_content(content).unwrap();
}
pub(crate) fn manager(names: &[&str]) -> AudioManager {
    let mut audio = AudioManager::new();
    audio.init();
    for name in names {
        audio.register_audio_event_info(AudioEventInfo {
            audio_name: (*name).into(),
            sound_type: AudioType::SoundEffect,
            sound_type_field: AudioType::SoundEffect,
            type_field: ST_GLOBAL,
            sounds: vec![format!("{name}.wav")],
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            volume: 1.0,
            volume_shift: 0.0,
            delay_min: 0.0,
            delay_max: 0.0,
            loop_count: 1,
            ..Default::default()
        });
    }
    audio
}
pub(crate) fn assert_events(audio: &AudioManager, expected: &[(&str, ObjectId)]) {
    let actual: Vec<_> = audio
        .pending_play_events()
        .map(|event| (event.get_event_name(), event.get_object_id()))
        .collect();
    let expected: Vec<_> = expected.iter().map(|(name, id)| (*name, id.0)).collect();
    assert_eq!(
        actual, expected,
        "real immediate AR_Play admissions preserve cue, owner and order"
    );
}
