//! Actual manager records: one owner's reset never retires peer receipts.
use super::*;
fn event(handle: u32, name: &str) -> AudioEventRts {
    let mut value = AudioEventRts::with_event_name(name);
    value.set_playing_handle(handle);
    value
}
#[test]
fn scoped_reset_retires_only_owned_active_pending_fading_and_force_records() {
    let mut device = AudioManager::new();
    device.script_music_volume = 0.37;
    device
        .active_audio_events
        .insert(1001, event(1001, "OwnerActive"));
    device
        .active_audio_events
        .insert(1002, event(1002, "PeerActive"));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1003, "OwnerPending"),
    ));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1004, "PeerPending"),
    ));
    device
        .audio_requests
        .push(AudioRequest::new_with_handle(RequestType::Stop, 1003));
    device
        .audio_requests
        .push(AudioRequest::new_with_handle(RequestType::Stop, 1004));
    device.fading_audio.push(FadingAudio {
        event: event(1005, "OwnerFade"),
        frames_faded: 3,
    });
    device.fading_audio.push(FadingAudio {
        event: event(1006, "PeerFade"),
        frames_faded: 9,
    });
    device
        .audio_force_played
        .extend([event(1007, "OwnerForce"), event(1008, "PeerForce")]);
    device.retire_audio_handles(&[1001, 1003, 1005, 1007]);
    assert_eq!(
        device
            .active_audio_events
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![1002]
    );
    assert_eq!(
        device
            .pending_play_events()
            .map(AudioEventRts::get_playing_handle)
            .collect::<Vec<_>>(),
        vec![1004]
    );
    assert_eq!(device.audio_requests.len(), 2);
    assert_eq!(device.audio_requests[1].get_handle(), Some(1004));
    assert_eq!(device.fading_audio.len(), 1);
    assert_eq!(device.fading_audio[0].event.get_playing_handle(), 1006);
    assert_eq!(device.fading_audio[0].frames_faded, 9);
    assert_eq!(
        device
            .audio_force_played
            .iter()
            .map(AudioEventRts::get_playing_handle)
            .collect::<Vec<_>>(),
        vec![1008]
    );
    assert_eq!(
        device.script_music_volume, 0.37,
        "per-world receipt retirement is not application policy reset"
    );
    // The explicit whole-device reset remains intact for application teardown.
    device.reset();
    assert_eq!(device.active_event_count(), 0);
    assert_eq!(device.pending_play_request_count(), 0);
    assert_eq!(device.fading_audio_count(), 0);
    assert!(device.audio_force_played.is_empty());
    assert_eq!(device.script_music_volume, 1.0);
}

#[test]
fn scoped_name_stop_preserves_peer_and_original_immediate_release_phase() {
    let mut device = AudioManager::new();
    device
        .active_audio_events
        .insert(1001, event(1001, "SharedLoop"));
    device
        .active_audio_events
        .insert(1002, event(1002, "SharedLoop"));
    device
        .active_audio_events
        .insert(1003, event(1003, "DifferentLoop"));
    device.audio_requests.push(AudioRequest::new_with_event(
        RequestType::Play,
        event(1004, "SharedLoop"),
    ));
    device.remove_owned_playing_audio("SharedLoop", &[1001, 1003, 1004]);
    assert!(!device.active_audio_events.contains_key(&1001));
    assert!(device.active_audio_events.contains_key(&1002));
    assert!(device.active_audio_events.contains_key(&1003));
    assert_eq!(
        device
            .pending_play_events()
            .map(AudioEventRts::get_playing_handle)
            .collect::<Vec<_>>(),
        vec![1004]
    );
    assert!(
        !device
            .audio_requests
            .iter()
            .any(|request| request.request == RequestType::Stop
                && request.get_handle() == Some(1001))
    );
    assert!(
        !device
            .audio_requests
            .iter()
            .any(|request| request.request == RequestType::Stop
                && request.get_handle() == Some(1002))
    );
}

#[test]
fn ambient_playing_observation_filters_shared_device_to_receiving_receipts() {
    let mut device = AudioManager::new();
    let mut owner = event(1001, "SharedAmbient");
    owner.set_object_id(77);
    let mut peer = event(1002, "SharedAmbient");
    peer.set_object_id(77);
    device.active_audio_events.insert(1001, owner);
    device.active_audio_events.insert(1002, peer);

    assert!(device.is_owned_named_event_playing_for_object(77, "SharedAmbient", &[1001]));
    assert!(device.is_owned_named_event_playing_for_object(77, "SharedAmbient", &[1002]));
    assert!(!device.is_owned_named_event_playing_for_object(77, "SharedAmbient", &[]));

    device.active_audio_events.remove(&1001);
    assert!(!device.is_owned_named_event_playing_for_object(77, "SharedAmbient", &[1001]));
    assert!(device.is_owned_named_event_playing_for_object(77, "SharedAmbient", &[1002]));
}
