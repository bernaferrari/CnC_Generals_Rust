use super::*;
use crate::common::audio::AudioInstance;

fn accepted(handle: AudioHandle) {
    assert!(
        (AHSV_FIRST_HANDLE..0xFFFF_FFF0).contains(&handle),
        "audio handle {handle}"
    );
}

#[test]
fn shared_device_limits_and_interrupts_only_the_receiving_cue() {
    let mut mixer = AudioManager::new();
    mixer.init();
    let left = AudioInstance::default();
    let right = AudioInstance::default();
    let cue = |instance: &AudioInstance, control| {
        let mut cue = event_with(
            test_info("SharedCue", AudioType::SoundEffect, control, 1),
            1.0,
        );
        cue.admit_instance(instance);
        cue
    };

    let left_handle = mixer.add_audio_event(&cue(&left, 0));
    accepted(left_handle);
    assert_eq!(mixer.add_audio_event(&cue(&left, 0)), AHSV_NO_SOUND);
    let right_handle = mixer.add_audio_event(&cue(&right, 0));
    accepted(right_handle);
    mixer.process_request_list();
    assert_eq!(mixer.active_event_count(), 2);

    let replacement = mixer.add_audio_event(&cue(&right, AC_INTERRUPT));
    accepted(replacement);
    assert_eq!(
        mixer.add_audio_event(&cue(&right, AC_INTERRUPT)),
        AHSV_NO_SOUND
    );
    assert_eq!(
        mixer
            .pending_play_events()
            .next()
            .unwrap()
            .get_handle_to_kill(),
        right_handle
    );
    mixer.process_request_list();
    assert!(mixer.active_audio_events.contains_key(&left_handle));
    assert!(mixer.active_audio_events.contains_key(&replacement));
    assert!(!mixer.active_audio_events.contains_key(&right_handle));
}

#[test]
fn shared_device_voice_culling_treats_same_object_id_as_instance_local() {
    let mut mixer = AudioManager::new();
    mixer.init();
    let left = AudioInstance::default();
    let right = AudioInstance::default();
    let voice = |instance: &AudioInstance, name| {
        let mut voice = voice_order_event(name, 17, true, false, 0.0);
        voice.admit_instance(instance);
        voice
    };
    let left_voice = mixer.add_audio_event(&voice(&left, "Acknowledge"));
    accepted(left_voice);
    mixer.process_request_list();
    assert_eq!(
        mixer.add_audio_event(&voice(&left, "AnotherOrder")),
        AHSV_NO_SOUND
    );
    let right_voice = mixer.add_audio_event(&voice(&right, "AnotherOrder"));
    accepted(right_voice);
}

#[test]
fn shared_device_speech_interrupt_and_lock_follow_the_receiving_instance() {
    let mut mixer = AudioManager::new();
    mixer.init();
    let left = AudioInstance::default();
    let right = AudioInstance::default();
    let speech = |instance: &AudioInstance, uninterruptable| {
        let mut speech = event_with(test_info("Briefing", AudioType::Streaming, 0, 0), 1.0);
        speech.admit_instance(instance);
        speech.set_uninterruptable(uninterruptable);
        speech
    };
    let left_voice = mixer.add_audio_event(&speech(&left, false));
    accepted(left_voice);
    mixer.process_request_list();
    let lock = mixer.add_audio_event(&speech(&right, true));
    accepted(lock);
    mixer.process_request_list();
    assert!(mixer.active_audio_events.contains_key(&left_voice));
    assert_eq!(mixer.add_audio_event(&speech(&right, false)), AHSV_NO_SOUND);
    accepted(mixer.add_audio_event(&speech(&left, false)));
    mixer.retire_audio_handles(&[lock]);
    accepted(mixer.add_audio_event(&speech(&right, false)));
}

#[test]
fn delayed_cue_rechecks_its_own_limit_on_the_original_request_tick() {
    let mut mixer = AudioManager::new();
    mixer.init();
    let left = AudioInstance::default();
    let right = AudioInstance::default();
    let cue = |scope: &AudioInstance, delay| {
        let mut info = test_info("DelayedSharedCue", AudioType::SoundEffect, 0, 2);
        info.delay_min = delay;
        info.delay_max = delay;
        let mut event = event_with(info, 1.0);
        event.admit_instance(scope);
        event
    };
    accepted(mixer.add_audio_event(&cue(&left, 0.0)));
    mixer.process_request_list();
    let delayed = mixer.add_audio_event(&cue(&right, 40.0));
    accepted(delayed);
    mixer.process_request_list();
    assert_eq!(mixer.pending_play_request_count(), 1);
    assert_eq!(mixer.active_event_count(), 1);
    mixer.process_request_list();
    assert_eq!(mixer.pending_play_request_count(), 0);
    assert!(mixer.active_audio_events.contains_key(&delayed));
}

#[test]
fn physical_sample_capacity_remains_shared_and_retirement_releases_the_slot() {
    let mut mixer = AudioManager::new();
    mixer.init();
    mixer.audio_settings.sample_count_2d = 1;
    mixer
        .sound_manager
        .as_mut()
        .unwrap()
        .configure_sample_capacity(1, 1);
    let left = AudioInstance::default();
    let right = AudioInstance::default();
    let cue = |scope: &AudioInstance, priority| {
        let mut info = test_info("SharedSamplePool", AudioType::SoundEffect, 0, 0);
        info.priority = priority;
        let mut event = event_with(info, 1.0);
        event.admit_instance(scope);
        event
    };
    let occupying = mixer.add_audio_event(&cue(&left, AudioPriority::Lowest));
    accepted(occupying);
    mixer.process_request_list();
    assert_eq!(
        mixer.add_audio_event(&cue(&right, AudioPriority::Critical)),
        AHSV_NO_SOUND
    );
    assert!(mixer.active_audio_events.contains_key(&occupying));
    mixer.retire_audio_handles(&[occupying]);
    let replacement = mixer.add_audio_event(&cue(&right, AudioPriority::Critical));
    accepted(replacement);
    mixer.process_request_list();
    assert!(mixer.active_audio_events.contains_key(&replacement));
}
