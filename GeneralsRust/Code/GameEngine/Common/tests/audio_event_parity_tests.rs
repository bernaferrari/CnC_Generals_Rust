use game_engine::common::audio::audio_event_rts::{
    AudioEventInfo, AudioEventRts, AudioPriority, AudioType,
};
use std::sync::Arc;

fn make_info(audio_name: &str) -> AudioEventInfo {
    AudioEventInfo {
        sound_type: AudioType::SoundEffect,
        control: 0,
        audio_name: audio_name.to_string(),
        volume: 1.0,
        sounds: vec!["ui_click".to_string()],
        attack_sounds: Vec::new(),
        decay_sounds: Vec::new(),
        pitch_shift_min: 1.0,
        pitch_shift_max: 1.0,
        volume_shift: 0.0,
        min_volume: 0.0,
        limit: 0,
        loop_count: 1,
        delay_min: 0.0,
        delay_max: 0.0,
        filename: "ui_click.wav".to_string(),
        sound_type_field: AudioType::SoundEffect,
        type_field: 0,
        priority: AudioPriority::Normal,
        min_distance: 0.0,
        max_distance: 1000.0,
    }
}

#[test]
fn get_audio_event_info_rejects_stale_cached_info() {
    let mut event = AudioEventRts::with_event_name("EventA");
    event.set_audio_event_info(Arc::new(make_info("DifferentEvent")));

    assert!(
        event.get_audio_event_info().is_none(),
        "stale cached info should be ignored when names differ"
    );
}

#[test]
fn get_audio_event_info_returns_matching_cached_info() {
    let mut event = AudioEventRts::with_event_name("EventA");
    event.set_audio_event_info(Arc::new(make_info("EventA")));

    let resolved = event
        .get_audio_event_info()
        .expect("matching cached info should resolve");
    assert_eq!(resolved.audio_name, "EventA");
}
