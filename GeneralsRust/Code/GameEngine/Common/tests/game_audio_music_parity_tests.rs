use game_engine::common::audio::{
    AC_INTERRUPT, AC_LOOP, AHSV_STOP_THE_MUSIC_FADE, AudioAffect, AudioEventInfo, AudioEventRts,
    AudioFileCache, AudioManager, AudioPriority, AudioRequest, AudioType, Coord3D, OwnerType,
    PortionToPlay, RequestType, ST_GLOBAL, ST_WORLD, music_repeats_source_infinitely,
};
use std::sync::Arc;

fn info(
    name: &str,
    sound_type: AudioType,
    control: u32,
    type_field: u32,
    loop_count: i32,
) -> AudioEventInfo {
    AudioEventInfo {
        sound_type,
        control,
        audio_name: name.to_string(),
        volume: 1.0,
        sounds_morning: Vec::new(),
        sounds: vec!["loop.wav".to_string()],
        sounds_night: Vec::new(),
        sounds_evening: Vec::new(),
        attack_sounds: Vec::new(),
        decay_sounds: Vec::new(),
        pitch_shift_min: 1.0,
        pitch_shift_max: 1.0,
        volume_shift: 0.0,
        min_volume: 0.0,
        limit: 0,
        loop_count,
        delay_min: 0.0,
        delay_max: 0.0,
        filename: "theme.wav".to_string(),
        sound_type_field: sound_type,
        type_field,
        priority: AudioPriority::Normal,
        min_distance: 25.0,
        max_distance: 100.0,
        ..Default::default()
    }
}

fn event_from(info: AudioEventInfo, handle: u32) -> AudioEventRts {
    let mut event = AudioEventRts::new();
    event.set_event_name(info.audio_name.clone());
    event.set_playing_handle(handle);
    event.set_volume(1.0);
    event.set_volume_shift(1.0);
    event.set_loop_count(info.loop_count);
    event.set_next_play_portion(PortionToPlay::Sound);
    event.set_audio_event_info(Arc::new(info));
    event
}

#[test]
fn music_track_navigation_updates_audio_manager_current_track() {
    let mut audio = AudioManager::new();
    audio.add_track_name("TrackA".to_string());
    audio.add_track_name("TrackB".to_string());
    audio.add_track_name("TrackC".to_string());

    audio.set_music_track_name("TrackA".to_string());
    assert_eq!(audio.next_music_track(), "TrackB");
    assert_eq!(audio.get_music_track_name(), "TrackB");

    assert_eq!(audio.prev_music_track(), "TrackA");
    assert_eq!(audio.get_music_track_name(), "TrackA");

    audio.set_music_track_name("Unknown".to_string());
    assert_eq!(audio.next_music_track(), "TrackA");
    assert_eq!(audio.prev_music_track(), "TrackC");
}

#[test]
fn music_source_is_marked_infinite_loop_like_miles_play_stream() {
    // C++ MilesAudioManager::playStream (MilesAudioManager.cpp:2762-2764)
    // AIL_set_stream_loop_count(stream, INFINITE_LOOP_COUNT) for AT_Music.
    let music = event_from(info("Theme", AudioType::Music, 0, 0, 1), 1001);
    let sfx = event_from(info("Boom", AudioType::SoundEffect, 0, 0, 1), 1002);
    assert!(music_repeats_source_infinitely(&music));
    assert!(!music_repeats_source_infinitely(&sfx));
}

#[test]
fn stop_the_music_fade_ramps_instead_of_immediate_stop() {
    // C++ MilesAudioManager::stopAudioEvent + processFadingList
    // (MilesAudioManager.cpp:885-907, 2410-2458). Fade sentinel must not
    // release the stream on the same frame.
    let mut audio = AudioManager::new();
    audio.insert_playing_event_for_test(event_from(info("Theme", AudioType::Music, 0, 0, 1), 1001));
    assert_eq!(audio.active_event_count(), 1);

    audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
    assert_eq!(audio.fading_audio_count(), 1);
    assert_eq!(audio.active_event_count(), 0);

    audio.update();
    assert_eq!(
        audio.fading_audio_count(),
        1,
        "AHSV_StopTheMusicFade must still be fading after one frame"
    );
    assert_eq!(audio.fading_frames(), 1);
}

#[test]
fn pause_audio_discards_pending_play_requests() {
    // C++ MilesAudioManager::pauseAudio (MilesAudioManager.cpp:569-583)
    // erases AR_Play from m_audioRequests.
    let mut audio = AudioManager::new();
    let event = event_from(info("Boom", AudioType::SoundEffect, 0, 0, 1), 1001);
    audio.append_audio_request(AudioRequest::new_with_event(RequestType::Play, event));
    assert_eq!(audio.pending_play_request_count(), 1);

    audio.pause_audio(AudioAffect::All);
    assert_eq!(audio.pending_play_request_count(), 0);
}

#[test]
fn process_playing_list_consumes_volume_has_changed() {
    // C++ MilesAudioManager::processPlayingList (MilesAudioManager.cpp:2266-2368)
    // applies adjustPlayingVolume then clears m_volumeHasChanged.
    let mut audio = AudioManager::new();
    audio.set_volume(0.25, AudioAffect::Music);
    assert!(audio.volume_has_changed_flag());

    audio.update();
    assert!(
        !audio.volume_has_changed_flag(),
        "volume_has_changed must be consumed by processPlayingList"
    );
}

#[test]
fn ac_loop_decreases_count_and_restarts_sound_portion() {
    // C++ MilesAudioManager::notifyOfAudioCompletion (MilesAudioManager.cpp:1519-1530)
    // AC_LOOP + PP_Sound → decreaseLoopCount + startNextLoop.
    let mut audio = AudioManager::new();
    let mut event = event_from(info("Ambient", AudioType::SoundEffect, AC_LOOP, 0, 2), 1001);
    event.set_loop_count(2);
    event.set_next_play_portion(PortionToPlay::Sound);
    audio.insert_playing_event_for_test(event);

    assert!(audio.force_notify_completion_for_test(1001));
    assert_eq!(audio.active_event_loop_count(1001), Some(1));
    assert_eq!(audio.active_event_portion(1001), Some(PortionToPlay::Sound));

    assert!(!audio.force_notify_completion_for_test(1001));
    assert_eq!(audio.active_event_count(), 0);
}

#[test]
fn attack_portion_advances_to_sound_before_loop_restart() {
    // C++ notifyOfAudioCompletion: AC_LOOP + PP_Attack sets PP_Sound then loops.
    let mut audio = AudioManager::new();
    let mut event = event_from(info("Ambient", AudioType::SoundEffect, AC_LOOP, 0, 3), 1001);
    event.set_loop_count(3);
    event.set_next_play_portion(PortionToPlay::Attack);
    audio.insert_playing_event_for_test(event);

    assert!(audio.force_notify_completion_for_test(1001));
    assert_eq!(audio.active_event_portion(1001), Some(PortionToPlay::Sound));
    assert_eq!(audio.active_event_loop_count(1001), Some(2));
}

#[test]
fn dead_owner_and_min_volume_cull_playing_3d() {
    // C++ MilesAudioManager::processPlayingList (MilesAudioManager.cpp:2296-2317)
    // stops isDead() owners and culls below m_minVolume unless ST_GLOBAL / AP_CRITICAL.
    let mut audio = AudioManager::new();
    audio.init();

    let mut dead = event_from(info("Engine", AudioType::SoundEffect, 0, ST_WORLD, 1), 1001);
    dead.set_object_id(7);
    dead.owner_type = OwnerType::Dead;
    audio.insert_playing_event_for_test(dead);
    audio.update();
    assert_eq!(audio.active_event_count(), 0, "dead owner must be released");

    let mut quiet = event_from(
        info("EngineFar", AudioType::SoundEffect, 0, ST_WORLD, 1),
        1002,
    );
    quiet.set_position(&Coord3D {
        x: 10_000.0,
        y: 0.0,
        z: 0.0,
    });
    audio.insert_playing_event_for_test(quiet);
    audio.update();
    assert_eq!(
        audio.active_event_count(),
        0,
        "sub-min-volume 3D must be culled"
    );

    let mut global = event_from(
        info(
            "EngineGlobal",
            AudioType::SoundEffect,
            0,
            ST_WORLD | ST_GLOBAL,
            1,
        ),
        1003,
    );
    global.set_position(&Coord3D {
        x: 10_000.0,
        y: 0.0,
        z: 0.0,
    });
    audio.insert_playing_event_for_test(global);
    audio.update();
    assert_eq!(
        audio.active_event_count(),
        1,
        "ST_GLOBAL must survive min-volume cull"
    );
}

fn event_named(info: AudioEventInfo, volume: f32) -> AudioEventRts {
    let mut event = AudioEventRts::with_event_name(&info.audio_name);
    event.set_audio_event_info(Arc::new(info));
    event.set_volume(volume);
    event
}

#[test]
fn uninterruptable_streaming_stops_speech_and_sets_disallow() {
    // C++ MilesAudioManager::playAudioEvent (MilesAudioManager.cpp:663-709)
    // AT_Streaming + uninterruptable → stopAllSpeech + setDisallowSpeech(TRUE).
    let mut manager = AudioManager::new();
    manager.init();

    let mut playing = event_from(info("BriefingA", AudioType::Streaming, 0, 0, 1), 1001);
    manager.insert_playing_event_for_test(playing);
    assert_eq!(manager.active_event_count(), 1);

    let mut incoming = event_named(info("BriefingB", AudioType::Streaming, 0, 0, 1), 1.0);
    incoming.set_uninterruptable(true);
    let handle = manager.add_audio_event(&incoming);
    assert_ne!(handle, 0);
    assert!(!manager.get_disallow_speech());
    assert_eq!(manager.pending_play_request_count(), 1);

    manager.process_request_list();
    assert!(manager.get_disallow_speech());
    assert_eq!(manager.active_event_count(), 1);
    assert!(manager.active_event_mut_for_test(1001).is_none());

    let blocked = event_named(info("BriefingC", AudioType::Streaming, 0, 0, 1), 1.0);
    assert_eq!(manager.add_audio_event(&blocked), 0);

    assert!(!manager.force_notify_completion_for_test(handle));
    assert!(!manager.get_disallow_speech());
}

#[test]
fn delayed_play_request_defers_until_under_one_logic_frame() {
    // C++ MilesAudioManager::shouldProcessRequestThisFrame / adjustRequest
    // (MilesAudioManager.cpp:2477-2498): delay >= MSEC_PER_LOGICFRAME_REAL is
    // decremented per frame; checkForSample is armed on the deferred request.
    let mut manager = AudioManager::new();
    manager.init();

    let mut delayed = info("DelayedBoom", AudioType::SoundEffect, 0, 0, 1);
    delayed.delay_min = 40.0;
    delayed.delay_max = 40.0;
    let event = event_named(delayed, 1.0);
    let _handle = manager.add_audio_event(&event);
    assert_eq!(manager.pending_play_request_count(), 1);
    assert_eq!(manager.active_event_count(), 0);
    assert!((manager.pending_play_delay().unwrap() - 40.0).abs() < 0.01);

    manager.process_request_list();
    assert_eq!(manager.pending_play_request_count(), 1);
    assert_eq!(manager.active_event_count(), 0);
    let remaining = manager.pending_play_delay().unwrap();
    assert!(remaining < 40.0);
    assert!(remaining >= 0.0);

    manager.process_request_list();
    assert_eq!(manager.pending_play_request_count(), 0);
    assert_eq!(manager.active_event_count(), 1);
}

#[test]
fn does_violate_limit_sets_oldest_handle_and_interrupt_request_counts() {
    // C++ MilesAudioManager::doesViolateLimit (MilesAudioManager.cpp:1802-1882).
    let mut manager = AudioManager::new();
    manager.init();

    let mut first = info("Boom", AudioType::SoundEffect, 0, 0, 1);
    first.limit = 1;
    let mut playing = event_from(first, 1001);
    manager.insert_playing_event_for_test(playing);

    let mut probe_info = info("Boom", AudioType::SoundEffect, 0, 0, 1);
    probe_info.limit = 1;
    let mut probe = event_named(probe_info, 1.0);
    assert!(manager.does_violate_limit(&mut probe));
    assert_eq!(probe.get_handle_to_kill(), 1001);

    let mut interrupt_info = info("Boom", AudioType::SoundEffect, AC_INTERRUPT, 0, 1);
    interrupt_info.limit = 1;
    let mut interrupt = event_named(interrupt_info, 1.0);
    assert!(!manager.does_violate_limit(&mut interrupt));
    assert_eq!(interrupt.get_handle_to_kill(), 1001);

    let mut queued_info = info("Boom", AudioType::SoundEffect, 0, 0, 1);
    queued_info.limit = 1;
    manager.append_audio_request(AudioRequest::new_with_event(
        RequestType::Play,
        event_named(queued_info, 1.0),
    ));
    let mut second_info = info("Boom", AudioType::SoundEffect, AC_INTERRUPT, 0, 1);
    second_info.limit = 1;
    let mut second_interrupt = event_named(second_info, 1.0);
    assert!(manager.does_violate_limit(&mut second_interrupt));
}

#[test]
fn next_music_track_stops_playing_and_queues_next() {
    // C++ MilesAudioManager::nextMusicTrack / getMusicTrackName
    // (MilesAudioManager.cpp:1313-1331, 1389-1416).
    let mut manager = AudioManager::new();
    manager.init();
    manager.add_track_name("TrackA".to_string());
    manager.add_track_name("TrackB".to_string());
    manager.register_audio_event_info(info("TrackA", AudioType::Music, 0, 0, 1));
    manager.register_audio_event_info(info("TrackB", AudioType::Music, 0, 0, 1));

    manager
        .insert_playing_event_for_test(event_from(info("TrackA", AudioType::Music, 0, 0, 1), 1001));
    assert_eq!(manager.get_music_track_name(), "TrackA");

    let next = manager.next_music_track();
    assert_eq!(next, "TrackB");
    assert_eq!(manager.get_music_track_name(), "TrackB");
    assert_eq!(manager.active_event_count(), 0);
    assert_eq!(manager.pending_play_request_count(), 1);
}

#[test]
fn friend_force_play_uses_speech_slider_and_tracks_handle() {
    // C++ MilesAudioManager::friend_forcePlayAudioEventRTS
    // (MilesAudioManager.cpp:2971-3017).
    let mut manager = AudioManager::new();
    manager.init();
    manager.set_volume(0.5, AudioAffect::SpeechSystemSetting);

    let briefing = event_named(info("MissionBrief", AudioType::Streaming, 0, 0, 1), 0.8);
    manager.friend_force_play_audio_event_rts(&briefing);
    assert_eq!(manager.force_played_count(), 1);
    assert!((manager.force_played_volume().unwrap() - 0.8).abs() < 1e-5);

    manager.reset();
    assert_eq!(manager.force_played_count(), 0);
}

#[test]
fn audio_file_cache_refcounts_named_buffers() {
    // C++ AudioFileCache::openFile (MilesAudioManager.cpp:3123-3128):
    // a second open of the same name increments m_openCount and returns
    // the same buffer pointer.
    let cache = AudioFileCache::new(1024);
    let first = cache
        .get_or_insert_named("boom.wav", || Some(vec![1, 2, 3, 4]))
        .expect("first insert");
    let second = cache
        .get_or_insert_named("boom.wav", || panic!("loader must not run on cache hit"))
        .expect("cache hit");
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    let cached = cache.get_cached_files();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].2, 2);
    cache.close_named("boom.wav");
    assert_eq!(cache.get_cached_files()[0].2, 1);
}

#[test]
fn ui_sfx_add_audio_event_queues_play_like_the_audio() {
    // C++ AudioManager::addAudioEvent (GameAudio.cpp) is TheAudio's one play API.
    // Pre-fix Main host_play_sound_effect / assets::audio::play_sound_effect
    // opened a local rodio sink and never queued AR_Play here.
    let mut audio = AudioManager::new();
    let before = audio.pending_play_request_count();
    let _ = audio.new_audio_event_info("UnitSelect".to_string());
    let event = AudioEventRts::with_event_name("UnitSelect");
    let handle = audio.add_audio_event(&event);
    assert!(
        handle >= 1000,
        "C++ AHSV_FIRST_HANDLE is 1000; got {handle}"
    );
    assert!(
        audio.pending_play_request_count() > before,
        "UI SFX must queue AR_Play on Common AudioManager"
    );
}

#[test]
fn initialize_global_audio_manager_applies_retail_slider_volumes() {
    // C++ AudioManager::init (GameAudio.cpp) copies preferred slider volumes.
    // Pre-fix `new()` left music/sound/speech at 0 so playback was muted.
    let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
    let guard = manager.lock().expect("THE_AUDIO lock");
    assert!(
        guard.get_volume(AudioAffect::Music) > 0.0
            && guard.get_volume(AudioAffect::Sound) > 0.0
            && guard.get_volume(AudioAffect::Sound3D) > 0.0
            && guard.get_volume(AudioAffect::Speech) > 0.0,
        "live THE_AUDIO must init retail sliders, not stay at 0"
    );
}

#[test]
fn update_drains_queued_play_requests_like_miles_process_request_list() {
    // C++ MilesAudioManager::update (MilesAudioManager.cpp:460-468)
    // processRequestList plays queued AR_Play. Live run_loop must call this.
    let mut audio = AudioManager::new();
    audio.init();
    let _ = audio.new_audio_event_info("UnitSelect".to_string());
    let event = AudioEventRts::with_event_name("UnitSelect");
    let handle = audio.add_audio_event(&event);
    assert!(handle >= 1000);
    assert!(audio.pending_play_request_count() > 0);
    audio.update();
    assert_eq!(
        audio.pending_play_request_count(),
        0,
        "AudioManager::update must process AR_Play"
    );
}
