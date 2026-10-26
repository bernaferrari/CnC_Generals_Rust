//! Source-phase and same-device peer isolation of real receiving mixer records.
use super::*;
use crate::common::audio::script_policy::ScriptAudioPolicy;
fn event(handle: u32, policy: &ScriptAudioPolicy) -> AudioEventRts {
    let mut event = AudioEventRts::with_event_name("SharedCue");
    event.set_volume(0.8);
    event.set_playing_handle(handle);
    event.admit_script_policy(policy);
    event
}
#[test]
fn owned_mix_refreshes_all_receiving_continuations_without_peer_policy_change() {
    let mut device = AudioManager::new();
    let mut a = ScriptAudioPolicy::default();
    let b = ScriptAudioPolicy::default();
    device.active_audio_events.insert(1001, event(1001, &a));
    device.active_audio_events.insert(1002, event(1002, &b));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1003, &a),
    ));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1004, &b),
    ));
    device.fading_audio.push(FadingAudio {
        event: event(1005, &a),
        frames_faded: 3,
    });
    device.audio_force_played.push(event(1006, &a));
    a.set_volume(0.25, AudioAffect::SoundEffects);
    device.refresh_owned_script_volumes(&[1001, 1003, 1005, 1006], a.volumes);
    assert_eq!(
        device.active_audio_events[&1001].owned_script_volumes,
        Some(a.volumes)
    );
    assert_eq!(
        device.active_audio_events[&1002].owned_script_volumes,
        Some(b.volumes)
    );
    assert_eq!(
        device
            .pending_play_events()
            .map(|e| e.owned_script_volumes)
            .collect::<Vec<_>>(),
        vec![Some(a.volumes), Some(b.volumes)]
    );
    assert_eq!(
        device.fading_audio[0].event.owned_script_volumes,
        Some(a.volumes)
    );
    assert_eq!(device.fading_audio[0].frames_faded, 3);
    assert_eq!(
        device.audio_force_played[0].owned_script_volumes,
        Some(a.volumes)
    );
    assert_eq!(
        device
            .event_volume_sliders(&device.active_audio_events[&1001])
            .sound_volume,
        device.system_sound_volume * 0.25
    );
    assert_eq!(
        device
            .event_volume_sliders(&device.active_audio_events[&1002])
            .sound_volume,
        device.system_sound_volume
    );
}
#[test]
fn named_override_changes_active_only_and_restore_keeps_committed_raw_volume() {
    let mut device = AudioManager::new();
    let mut a = ScriptAudioPolicy::default();
    let b = ScriptAudioPolicy::default();
    device.active_audio_events.insert(1001, event(1001, &a));
    device.active_audio_events.insert(1002, event(1002, &b));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1003, &a),
    ));
    a.set_named_volume("SharedCue", 0.3);
    device.adjust_owned_playing_volume(&[1001, 1003], "SharedCue", 0.3);
    assert_eq!(device.active_audio_events[&1001].get_volume(), 0.3);
    assert_eq!(device.active_audio_events[&1002].get_volume(), 0.8);
    assert_eq!(
        device.pending_play_events().next().unwrap().get_volume(),
        0.8
    );
    a.set_named_volume("SharedCue", -1.);
    assert_eq!(device.active_audio_events[&1001].get_volume(), 0.3);
    assert_eq!(event(1004, &a).get_volume(), 0.8);
}
#[test]
fn pause_cancels_only_receiving_pending_plays_and_preserves_pending_stops() {
    let mut device = AudioManager::new();
    let policy = ScriptAudioPolicy::default();
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1001, &policy),
    ));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1002, &policy),
    ));
    device
        .audio_requests
        .push(AudioRequest::new_with_handle(RequestType::Stop, 1001));
    device.pause_owned_audio(&[1001], AudioAffect::Music);
    assert_eq!(
        device
            .pending_play_events()
            .map(AudioEventRts::get_playing_handle)
            .collect::<Vec<_>>(),
        vec![1002]
    );
    assert!(
        device
            .audio_requests
            .iter()
            .any(|r| r.request == RequestType::Stop && r.get_handle() == Some(1001))
    );
}

#[test]
fn pause_resume_source_categories_do_not_expand_partial_mask_to_all() {
    fn authored(kind: AudioType, spatial: bool) -> AudioEventRts {
        let mut event = AudioEventRts::with_event_name("CategoryCue");
        event.set_audio_event_info(std::sync::Arc::new(AudioEventInfo {
            audio_name: "CategoryCue".into(),
            sound_type: kind,
            type_field: if spatial {
                crate::common::audio::audio_event_rts::ST_WORLD
            } else {
                0
            },
            ..Default::default()
        }));
        if spatial {
            event.set_position(&Coord3D::ZERO);
        }
        event
    }
    let events = [
        authored(AudioType::Music, false),
        authored(AudioType::Streaming, false),
        authored(AudioType::SoundEffect, false),
        authored(AudioType::SoundEffect, true),
    ];
    // MilesAudioManager.cpp527-572/590-636: independent stream/2D/3D bits.
    for (mask, expected) in [
        (AudioAffect::Music, [true, false, false, false]),
        (AudioAffect::Speech, [false, true, false, false]),
        (AudioAffect::Sound, [false, false, true, false]),
        (AudioAffect::Sound3D, [false, false, false, true]),
        (AudioAffect::SoundEffects, [false, false, true, true]),
        (AudioAffect::All, [true, true, true, true]),
    ] {
        assert_eq!(
            events
                .each_ref()
                .map(|event| event_matches_audio_affect(event, mask)),
            expected,
            "wrong original category mask {mask:?}"
        );
    }
}

#[test]
fn owned_music_completion_ignores_same_name_peer_and_nonmusic_records() {
    struct CompletionBackend;
    impl SoundPlaybackHook for CompletionBackend {
        fn play(&self, _: &AudioEventRts) -> Result<(), String> {
            Err("completion observation fixture does not create physical playback".into())
        }
        fn stop(&self, _: AudioHandle) {}
        fn pause(&self, _: AudioHandle) {}
        fn resume(&self, _: AudioHandle) {}
        fn is_playing(&self, _: AudioHandle) -> bool {
            true
        }
        fn music_loop_count(&self, handle: AudioHandle) -> Int {
            match handle {
                1001 => 1,
                1002 => 4,
                1003 => 8,
                _ => 0,
            }
        }
    }
    let mut device = AudioManager::with_playback_hook(Box::new(CompletionBackend));
    for (handle, sound_type) in [
        (1001, AudioType::Music),
        (1002, AudioType::Music),
        (1003, AudioType::SoundEffect),
    ] {
        let mut event = AudioEventRts::with_event_name("SharedMusic");
        event.set_playing_handle(handle);
        event.set_audio_event_info(std::sync::Arc::new(AudioEventInfo {
            audio_name: "SharedMusic".into(),
            sound_type,
            ..Default::default()
        }));
        device.active_audio_events.insert(handle, event);
    }
    assert!(!device.has_owned_music_track_completed("SharedMusic", 2, &[1001]));
    assert!(device.has_owned_music_track_completed("SharedMusic", 1, &[1001]));
    assert!(device.has_owned_music_track_completed("SharedMusic", 4, &[1002]));
    assert!(!device.has_owned_music_track_completed("SharedMusic", 5, &[1002]));
    assert!(!device.has_owned_music_track_completed("SharedMusic", 2, &[1003]));
    assert!(!device.has_owned_music_track_completed("SharedMusic", 0, &[]));
    assert!(!device.has_owned_music_track_completed("OtherMusic", 0, &[1001, 1002]));
    device.active_audio_events.remove(&1002);
    assert!(!device.has_owned_music_track_completed("SharedMusic", 2, &[1001, 1002]));
    assert!(device.active_audio_events.contains_key(&1001));
}
