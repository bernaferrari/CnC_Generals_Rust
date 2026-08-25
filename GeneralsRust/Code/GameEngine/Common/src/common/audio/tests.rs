use super::*;

#[test]
fn audio_affect_system_setting_combinations_match_cpp_flags() {
    assert_eq!(
        AudioAffect::from_bits(AudioAffect::Music.bits() | AudioAffect::SystemSetting.bits()),
        Some(AudioAffect::MusicSystemSetting)
    );
    assert_eq!(
        AudioAffect::from_bits(AudioAffect::Sound.bits() | AudioAffect::Sound3D.bits()),
        Some(AudioAffect::SoundEffects)
    );
    assert_eq!(
        AudioAffect::from_bits(AudioAffect::All.bits() | AudioAffect::SystemSetting.bits()),
        Some(AudioAffect::AllSystemSetting)
    );

    let mut audio_manager = AudioManager::new();
    audio_manager.set_volume(0.25, AudioAffect::AllSystemSetting);
    assert_eq!(audio_manager.system_music_volume, 0.25);
    assert_eq!(audio_manager.system_sound_volume, 0.25);
    assert_eq!(audio_manager.system_sound_3d_volume, 0.25);
    assert_eq!(audio_manager.system_speech_volume, 0.25);

    audio_manager.set_on(false, AudioAffect::SoundEffects);
    assert!(!audio_manager.is_on(AudioAffect::Sound));
    assert!(!audio_manager.is_on(AudioAffect::Sound3D));
    assert!(audio_manager.is_on(AudioAffect::Music));
}

#[test]
fn audio_manager_get_effective_volume_matches_miles_category_and_3d() {
    let mut manager = AudioManager::new();
    manager.init();

    let info = AudioEventInfo {
        sound_type: AudioType::SoundEffect,
        control: 0,
        audio_name: "Boom".to_string(),
        volume: 1.0,
        sounds_morning: Vec::new(),
        sounds: Vec::new(),
        sounds_night: Vec::new(),
        sounds_evening: Vec::new(),
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
        filename: String::new(),
        sound_type_field: AudioType::SoundEffect,
        type_field: crate::common::audio::ST_WORLD,
        priority: AudioPriority::Normal,
        min_distance: 25.0,
        max_distance: 1000.0,
        ..Default::default()
    };

    let mut event = AudioEventRts::new();
    event.set_event_name("Boom".to_string());
    event.set_audio_event_info(std::sync::Arc::new(info));
    event.set_volume(1.0);
    event.set_volume_shift(1.0);
    event.set_position(&Coord3D {
        x: 50.0,
        y: 0.0,
        z: 0.0,
    });
    // preferred_3d 0.75 * 1/(50/25) = 0.375
    assert!((manager.get_effective_volume(&event) - 0.375).abs() < 1e-5);

    let mut music = AudioEventRts::new();
    music.set_event_name("Theme".to_string());
    music.set_audio_event_info(std::sync::Arc::new(AudioEventInfo {
        sound_type: AudioType::Music,
        control: 0,
        audio_name: "Theme".to_string(),
        volume: 0.8,
        sounds_morning: Vec::new(),
        sounds: Vec::new(),
        sounds_night: Vec::new(),
        sounds_evening: Vec::new(),
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
        filename: String::new(),
        sound_type_field: AudioType::Music,
        type_field: 0,
        priority: AudioPriority::Normal,
        min_distance: 25.0,
        max_distance: 1000.0,
        ..Default::default()
    }));
    music.set_volume(0.8);
    music.set_volume_shift(1.0);
    // 0.8 * 1.0 * preferred_music 0.55 = 0.44
    assert!((manager.get_effective_volume(&music) - 0.44).abs() < 1e-5);
}

fn test_info(name: &str, sound_type: AudioType, control: u32, limit: Int) -> AudioEventInfo {
    AudioEventInfo {
        sound_type,
        control,
        audio_name: name.to_string(),
        volume: 1.0,
        sounds_morning: Vec::new(),
        sounds: vec![format!("{name}.wav")],
        sounds_night: Vec::new(),
        sounds_evening: Vec::new(),
        attack_sounds: Vec::new(),
        decay_sounds: Vec::new(),
        pitch_shift_min: 1.0,
        pitch_shift_max: 1.0,
        volume_shift: 0.0,
        min_volume: 0.0,
        limit,
        loop_count: 1,
        delay_min: 0.0,
        delay_max: 0.0,
        filename: format!("{name}.wav"),
        sound_type_field: sound_type,
        type_field: 0,
        priority: AudioPriority::Normal,
        min_distance: 25.0,
        max_distance: 1000.0,
        ..Default::default()
    }
}

fn event_with(info: AudioEventInfo, volume: Real) -> AudioEventRts {
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

    let mut playing = event_with(test_info("BriefingA", AudioType::Streaming, 0, 0), 1.0);
    playing.set_playing_handle(1001);
    manager.insert_playing_event_for_test(playing);
    assert_eq!(manager.active_event_count(), 1);

    let mut incoming = event_with(test_info("BriefingB", AudioType::Streaming, 0, 0), 1.0);
    incoming.set_uninterruptable(true);
    let handle = manager.add_audio_event(&incoming);
    assert_ne!(handle, AHSV_NO_SOUND);
    assert!(!manager.get_disallow_speech());
    assert_eq!(manager.pending_play_request_count(), 1);

    manager.process_request_list();
    assert!(manager.get_disallow_speech());
    assert_eq!(manager.active_event_count(), 1);
    assert!(manager.active_event_mut_for_test(1001).is_none());

    let blocked = event_with(test_info("BriefingC", AudioType::Streaming, 0, 0), 1.0);
    assert_eq!(manager.add_audio_event(&blocked), AHSV_NO_SOUND);

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

    let mut info = test_info("DelayedBoom", AudioType::SoundEffect, 0, 0);
    info.delay_min = 40.0;
    info.delay_max = 40.0;
    let event = event_with(info, 1.0);
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

    let mut first = event_with(test_info("Boom", AudioType::SoundEffect, 0, 1), 1.0);
    first.set_playing_handle(1001);
    manager.insert_playing_event_for_test(first);

    let mut probe = event_with(test_info("Boom", AudioType::SoundEffect, 0, 1), 1.0);
    assert!(manager.does_violate_limit(&mut probe));
    assert_eq!(probe.get_handle_to_kill(), 1001);

    let mut interrupt = event_with(
        test_info("Boom", AudioType::SoundEffect, AC_INTERRUPT, 1),
        1.0,
    );
    assert!(!manager.does_violate_limit(&mut interrupt));
    assert_eq!(interrupt.get_handle_to_kill(), 1001);

    let queued = event_with(test_info("Boom", AudioType::SoundEffect, 0, 1), 1.0);
    let _ = manager.add_audio_event(&queued);
    let mut second_interrupt = event_with(
        test_info("Boom", AudioType::SoundEffect, AC_INTERRUPT, 1),
        1.0,
    );
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
    manager.register_audio_event_info(test_info("TrackA", AudioType::Music, 0, 0));
    manager.register_audio_event_info(test_info("TrackB", AudioType::Music, 0, 0));

    let mut playing = event_with(test_info("TrackA", AudioType::Music, 0, 0), 1.0);
    playing.set_playing_handle(1001);
    manager.insert_playing_event_for_test(playing);
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
    manager.set_volume(0.5, AudioAffect::Speech);

    let briefing = event_with(test_info("MissionBrief", AudioType::Streaming, 0, 0), 0.8);
    manager.friend_force_play_audio_event_rts(&briefing);
    assert_eq!(manager.force_played_count(), 1);
    assert!((manager.force_played_volume().unwrap() - 0.8).abs() < 1e-5);

    manager.reset();
    assert_eq!(manager.force_played_count(), 0);
}

#[test]
fn wav_channel_count_reads_fmt_channels() {
    // C++ AudioFileCache::openFile stereo check (MilesAudioManager.cpp:3147-3152).
    let mut wav = vec![0u8; 24];
    wav[0..4].copy_from_slice(b"RIFF");
    wav[22..24].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(wav_channel_count(&wav), Some(2));
    assert_eq!(wav_channel_count(b"xxxx"), None);
}

#[test]
fn the_audio_singleton_registers_rodio_not_wwaudio() {
    // C++ TheAudio (AudioManager) is created in GameEngine::init
    // (GameEngine.cpp createAudioManager / MilesAudioManager).
    // Live Rust analog is Common THE_AUDIO + rodio, not the leftover audio crate.
    let manager = initialize_global_audio_manager();
    assert!(
        THE_AUDIO.get().is_some(),
        "THE_AUDIO must be the live Common AudioManager singleton"
    );
    assert!(Arc::ptr_eq(
        &manager,
        THE_AUDIO.get().expect("THE_AUDIO set")
    ));
    let src = include_str!("game_audio.rs");
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(
        prod.contains("use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source}")
            && prod.contains("fn register_rodio_playback_hook"),
        "Common TheAudio backend must be rodio, not Miles leftover crate"
    );
    assert!(
        !prod.contains("use wwaudio") && !prod.contains("wp-audio"),
        "Common TheAudio must not import leftover WWAudio crate"
    );
}

#[test]
fn initialize_global_audio_manager_applies_retail_slider_volumes() {
    // C++ AudioManager::init (GameAudio.cpp) copies preferred slider volumes.
    // Pre-fix `new()` left music/sound/speech at 0 so playback was muted.
    let manager = initialize_global_audio_manager();
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
    assert!(handle >= AHSV_FIRST_HANDLE);
    assert!(audio.pending_play_request_count() > 0);
    audio.update();
    assert_eq!(
        audio.pending_play_request_count(),
        0,
        "AudioManager::update must process AR_Play"
    );
}

#[test]
fn boot_equivalent_audio_ini_load_resolves_retail_event_names() {
    // C++ AudioManager::init (GameAudio.cpp:186-199) loads Music.ini,
    // SoundEffects.ini, Speech.ini, Voice.ini into m_allAudioEventInfo.
    // addAudioEvent returns AHSV_Error when the name is missing (line 393-395).
    let manager = initialize_global_audio_manager();
    load_audio_event_inis();
    let mut guard = manager.lock().expect("THE_AUDIO lock");
    assert!(
        guard.find_audio_event_info("GUIClick").is_some(),
        "SoundEffects.ini GUIClick must register on THE_AUDIO"
    );
    assert!(
        guard.find_audio_event_info("RangerVoiceSelect").is_some(),
        "Voice.ini RangerVoiceSelect must register on THE_AUDIO"
    );
    assert!(
        guard
            .find_audio_event_info("EvaGLA_AllyUnderAttack")
            .is_some(),
        "Speech.ini EvaGLA_AllyUnderAttack must register on THE_AUDIO"
    );
    assert!(
        guard.find_audio_event_info("Shell").is_some(),
        "Music.ini Shell must register on THE_AUDIO"
    );

    let event = AudioEventRts::with_event_name("GUIClick");
    let handle = guard.add_audio_event(&event);
    assert_ne!(
        handle, AHSV_ERROR,
        "known retail event must not return AHSV_Error after boot INI load"
    );
    assert!(handle >= AHSV_FIRST_HANDLE);
}

#[test]
fn rodio_play_does_not_reenter_the_audio_mutex() {
    let src = include_str!("game_audio.rs");
    let play = src
        .split("impl SoundPlaybackHook for RodioPlaybackHook")
        .nth(1)
        .and_then(|s| s.split("fn stop(").next())
        .unwrap_or("");
    assert!(
        play.contains("try_lock()"),
        "Rodio play must try_lock THE_AUDIO sliders; C++ Miles never re-enters AudioManager from play"
    );
    assert!(
        !play.contains("manager.lock()"),
        "blocking THE_AUDIO lock from Rodio play deadlocks Menu TheAudio::update"
    );
}

#[test]
fn stop_all_speech_hard_releases_streaming_handles() {
    // C++ MilesAudioManager::stopAllSpeech (MilesAudioManager.cpp:1243-1260)
    // releasePlayingAudio + erase, not requestStop.
    let mut manager = AudioManager::new();
    manager.init();
    let mut playing = event_with(test_info("BriefingA", AudioType::Streaming, 0, 0), 1.0);
    playing.set_playing_handle(1001);
    manager.insert_playing_event_for_test(playing);
    manager.stop_all_speech();
    assert!(manager.active_event_mut_for_test(1001).is_none());
    assert_eq!(manager.active_event_count(), 0);
}

#[test]
fn play_audio_event_kills_oldest_same_name_immediately() {
    // C++ MilesAudioManager::playAudioEvent handleToKill (MilesAudioManager.cpp:735-813)
    // releasePlayingAudio hard-stops the channel before reuse.
    let mut manager = AudioManager::new();
    manager.init();
    let mut first = event_with(test_info("Boom", AudioType::SoundEffect, 0, 1), 1.0);
    first.set_playing_handle(1001);
    manager.insert_playing_event_for_test(first);

    let incoming = event_with(
        test_info("Boom", AudioType::SoundEffect, AC_INTERRUPT, 1),
        1.0,
    );
    let handle = manager.add_audio_event(&incoming);
    assert_ne!(handle, AHSV_NO_SOUND);
    manager.process_request_list();
    assert!(
        manager.active_event_mut_for_test(1001).is_none(),
        "oldest same-name handle must be hard-released, not left requestStop"
    );
}

#[test]
fn stop_audio_releases_playing_events() {
    // C++ MilesAudioManager::stopAudio sets PS_Stopped; processPlayingList
    // releasePlayingAudio. Paused Rodio sinks must not stay as zombies.
    let mut manager = AudioManager::new();
    manager.init();
    let mut music = event_with(test_info("Theme", AudioType::Music, 0, 0), 1.0);
    music.set_playing_handle(1001);
    manager.insert_playing_event_for_test(music);
    manager.stop_audio(AudioAffect::Music);
    assert_eq!(manager.active_event_count(), 0);
}

#[test]
fn has_3d_sensitive_streams_ignores_positional_sfx() {
    // C++ has3DSensitiveStreamsPlaying walks streams only (2381-2404).
    let mut manager = AudioManager::new();
    manager.init();

    let mut sfx_info = test_info("Gun", AudioType::SoundEffect, 0, 0);
    sfx_info.type_field = ST_WORLD;
    let mut sfx = event_with(sfx_info, 1.0);
    sfx.set_position(&Coord3D {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    });
    sfx.set_playing_handle(1001);
    manager.insert_playing_event_for_test(sfx);
    assert!(
        !manager.has_3d_sensitive_streams_playing(),
        "positional SFX are samples, not streams"
    );

    let mut speech = event_with(test_info("EvaAlert", AudioType::Streaming, 0, 0), 1.0);
    speech.set_playing_handle(1002);
    manager.insert_playing_event_for_test(speech);
    assert!(manager.has_3d_sensitive_streams_playing());
}

#[test]
fn game_underscore_music_is_not_3d_sensitive() {
    let mut manager = AudioManager::new();
    manager.init();
    let mut music = event_with(test_info("Game_Skirmish", AudioType::Music, 0, 0), 1.0);
    music.set_playing_handle(1001);
    manager.insert_playing_event_for_test(music);
    assert!(!manager.has_3d_sensitive_streams_playing());

    let mut cinematic = event_with(test_info("Cine_Intro", AudioType::Music, 0, 0), 1.0);
    cinematic.set_playing_handle(1002);
    manager.insert_playing_event_for_test(cinematic);
    assert!(manager.has_3d_sensitive_streams_playing());
}

#[test]
fn play_audio_event_steals_lowest_priority_when_pool_full() {
    // C++ getFirst3DSample empty → killLowestPrioritySoundImmediately.
    let mut manager = AudioManager::new();
    manager.init();
    manager.audio_settings.sample_count_3d = 1;

    let mut low_info = test_info("Ambient", AudioType::SoundEffect, 0, 0);

    low_info.type_field = ST_WORLD;
    low_info.priority = AudioPriority::Lowest;
    let mut low = event_with(low_info, 1.0);
    low.set_position(&Coord3D {
        x: 5.0,
        y: 0.0,
        z: 0.0,
    });
    low.set_playing_handle(1001);
    manager.insert_playing_event_for_test(low);
    let mut high_info = test_info("WeaponFire", AudioType::SoundEffect, 0, 0);
    high_info.type_field = ST_WORLD;
    high_info.priority = AudioPriority::High;
    let mut high = event_with(high_info, 1.0);
    high.set_position(&Coord3D {
        x: 8.0,
        y: 0.0,
        z: 0.0,
    });
    let handle = manager.add_audio_event(&high);
    assert_ne!(handle, AHSV_NO_SOUND);
    manager.process_request_list();
    assert!(
        manager.active_event_mut_for_test(1001).is_none(),
        "lowest-priority 3D sample must be evicted for the new higher-priority play"
    );
}

#[test]
fn is_playing_already_partitions_2d_from_3d() {
    // C++ MilesAudioManager::isPlayingAlready (1886-1905) scans only
    // m_playingSounds or m_playing3DSounds, never both.
    let mut manager = AudioManager::new();
    manager.init();

    let mut world = test_info("Boom", AudioType::SoundEffect, 0, 0);
    world.type_field = ST_WORLD;
    let mut playing_3d = event_with(world, 1.0);
    playing_3d.set_position(&Coord3D {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    });
    playing_3d.set_playing_handle(1001);
    manager.insert_playing_event_for_test(playing_3d);

    let probe_2d = event_with(test_info("Boom", AudioType::SoundEffect, 0, 0), 1.0);
    assert!(
        !probe_2d.is_positional_audio(),
        "UI/2D probe must not be positional"
    );
    assert!(
        !manager.is_playing_already(&probe_2d),
        "same-name 3D must not count as already-playing for a 2D AC_INTERRUPT"
    );

    let mut world_probe = test_info("Boom", AudioType::SoundEffect, 0, 0);
    world_probe.type_field = ST_WORLD;
    let mut probe_3d = event_with(world_probe, 1.0);
    probe_3d.set_position(&Coord3D {
        x: 20.0,
        y: 0.0,
        z: 0.0,
    });
    assert!(probe_3d.is_positional_audio());
    assert!(
        manager.is_playing_already(&probe_3d),
        "same-name 3D must count as already-playing in the 3D partition"
    );
}

#[test]
fn should_play_locally_player_allies_enemies_match_cpp() {
    // Missing restriction bits default Everyone.
    assert!(should_play_locally_for_players(
        0,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Enemies,
    ));
    assert!(should_play_locally_for_players(
        ST_EVERYONE,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Enemies,
    ));
    assert!(should_play_locally_for_players(
        ST_PLAYER,
        true,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Enemies,
    ));

    // ST_PLAYER: only the owning local player hears it.
    assert!(should_play_locally_for_players(
        ST_PLAYER,
        false,
        0,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Allies,
    ));
    assert!(!should_play_locally_for_players(
        ST_PLAYER,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Enemies,
    ));

    // ST_PLAYER+ST_UI with no owner still plays.
    assert!(should_play_locally_for_players(
        ST_PLAYER | ST_UI,
        false,
        -1,
        false,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Neutral,
    ));
    // ST_PLAYER without owner (and without ST_UI) does not.
    assert!(!should_play_locally_for_players(
        ST_PLAYER,
        false,
        -1,
        false,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Neutral,
    ));

    // ST_ALLIES: ally hears it, owner does not (PLAYER was not set).
    assert!(should_play_locally_for_players(
        ST_ALLIES,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Allies,
    ));
    assert!(!should_play_locally_for_players(
        ST_ALLIES,
        false,
        0,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Allies,
    ));

    // ST_ENEMIES: only an enemy relationship plays.
    assert!(should_play_locally_for_players(
        ST_ENEMIES,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Enemies,
    ));
    assert!(!should_play_locally_for_players(
        ST_ENEMIES,
        false,
        1,
        true,
        Some(0),
        true,
        None,
        true,
        AudioLocalityRelationship::Allies,
    ));
}

#[test]
fn set_listener_position_stores_orientation() {
    let mut manager = AudioManager::new();
    let pos = Coord3D {
        x: 10.0,
        y: 20.0,
        z: 30.0,
    };
    let ori = Coord3D {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    manager.set_listener_position(&pos, &ori);
    assert_eq!(manager.get_listener_position().x, 10.0);
    assert_eq!(manager.get_listener_orientation().y, 1.0);
    assert!(
        (stereo_pan(
            &pos,
            0.0,
            1.0,
            &Coord3D {
                x: 20.0,
                y: 20.0,
                z: 30.0
            }
        ) - 1.0)
            .abs()
            < 1e-5
    );
}
