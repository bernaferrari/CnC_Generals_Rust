// Characterization coverage for the mission script runtime and action dispatch.

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::scripting::core::{
        Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, ScriptActionType,
        ScriptGroup,
    };
    use gamelogic::scripting::engine::{
        ScriptEngine, ScriptEngineHandle, initialize_script_engine,
    };

    #[test]
    fn poisoned_notification_queue_preserves_existing_and_next_handler_actions() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        let handler = MissionScriptActionHandler::new(hooks.clone());
        handler.set_radar_forced(true).unwrap();
        hooks.poison_notifications_for_test();
        handler.set_radar_forced(false).unwrap();
        assert_eq!(hooks.drain_radar_forced_updates(), vec![true, false]);
        assert!(hooks.drain_radar_forced_updates().is_empty());
        assert!(!hooks.notifications.is_poisoned());
    }

    #[test]
    fn poisoned_notification_queue_recovers_grouped_camera_drain_and_clear() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        hooks.push_camera_move(Vec3::new(1.0, 2.0, 3.0));
        hooks.push_camera_move_home();
        hooks.queue_warehouse_set_value("Depot", 700);
        hooks.poison_notifications_for_test();
        let focus = hooks.take_camera_focus_requests();
        assert_eq!(focus.moves, vec![Vec3::new(1.0, 2.0, 3.0)]);
        assert_eq!(focus.move_home.len(), 1);
        assert!(hooks.take_camera_focus_requests().moves.is_empty());
        hooks.poison_notifications_for_test();
        hooks.clear_warehouse_set_values();
        assert!(hooks.drain_warehouse_set_values().is_empty());
        hooks.queue_warehouse_set_value("Depot", 900);
        assert_eq!(
            hooks.drain_warehouse_set_values(),
            vec![("Depot".into(), 900)]
        );
    }

    #[test]
    fn poisoned_notification_queue_is_local_to_its_hooks() {
        let first = MissionScriptHooks::new().expect("first hooks");
        let second = MissionScriptHooks::new().expect("second hooks");
        first.queue_warehouse_set_value("Depot", 100);
        second.queue_warehouse_set_value("Depot", 200);
        first.poison_notifications_for_test();
        assert!(!second.notifications.is_poisoned());
        assert_eq!(
            second.drain_warehouse_set_values(),
            vec![("Depot".into(), 200)]
        );
        assert_eq!(
            first.drain_warehouse_set_values(),
            vec![("Depot".into(), 100)]
        );
    }

    #[test]
    fn handler_forwards_camera_pitch_rotate_and_mod_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_camera_pitch(1.25, 2.0, 0.5, 0.25)
            .expect("pitch action should succeed");
        handler
            .rotate_camera(0.5, 3.0, 0.2, 0.4)
            .expect("rotate action should succeed");
        handler
            .camera_mod_set_final_zoom(0.8, 0.3, 0.1)
            .expect("camera mod final zoom should succeed");
        handler
            .camera_mod_set_final_pitch(1.1, 0.25, 0.15)
            .expect("camera mod final pitch should succeed");
        handler
            .camera_mod_freeze_time()
            .expect("camera mod freeze time should succeed");
        handler
            .camera_mod_freeze_angle()
            .expect("camera mod freeze angle should succeed");
        handler
            .camera_mod_set_final_speed_multiplier(4)
            .expect("camera mod final speed multiplier should succeed");
        handler
            .camera_mod_set_rolling_average(6)
            .expect("camera mod rolling average should succeed");
        handler
            .set_visual_speed_multiplier(3)
            .expect("visual speed multiplier should succeed");
        handler.freeze_time().expect("freeze time should succeed");
        handler
            .unfreeze_time()
            .expect("unfreeze time should succeed");
        handler
            .set_fps_limit(120)
            .expect("set fps limit should succeed");

        let pitch = hooks.drain_camera_pitch_requests();
        assert_eq!(pitch.len(), 1);
        assert!((pitch[0].pitch - 1.25).abs() < f32::EPSILON);
        assert!((pitch[0].duration_seconds - 2.0).abs() < f32::EPSILON);
        assert!((pitch[0].ease_in_seconds - 0.5).abs() < f32::EPSILON);
        assert!((pitch[0].ease_out_seconds - 0.25).abs() < f32::EPSILON);

        let rotate = hooks.drain_camera_rotate_requests();
        assert_eq!(rotate.len(), 1);
        assert!((rotate[0].rotations - 0.5).abs() < f32::EPSILON);
        assert!((rotate[0].duration_seconds - 3.0).abs() < f32::EPSILON);
        assert!((rotate[0].ease_in_seconds - 0.2).abs() < f32::EPSILON);
        assert!((rotate[0].ease_out_seconds - 0.4).abs() < f32::EPSILON);

        let final_zoom = hooks.drain_camera_mod_final_zoom_requests();
        assert_eq!(final_zoom.len(), 1);
        assert!((final_zoom[0].zoom - 0.8).abs() < f32::EPSILON);
        assert!((final_zoom[0].ease_in - 0.3).abs() < f32::EPSILON);
        assert!((final_zoom[0].ease_out - 0.1).abs() < f32::EPSILON);

        let final_pitch = hooks.drain_camera_mod_final_pitch_requests();
        assert_eq!(final_pitch.len(), 1);
        assert!((final_pitch[0].pitch - 1.1).abs() < f32::EPSILON);
        assert!((final_pitch[0].ease_in - 0.25).abs() < f32::EPSILON);
        assert!((final_pitch[0].ease_out - 0.15).abs() < f32::EPSILON);

        let freeze_time = hooks.drain_camera_mod_freeze_time_requests();
        assert_eq!(freeze_time.len(), 1);
        let freeze_angle = hooks.drain_camera_mod_freeze_angle_requests();
        assert_eq!(freeze_angle.len(), 1);
        let final_speed = hooks.drain_camera_mod_final_speed_multiplier_requests();
        assert_eq!(final_speed.len(), 1);
        assert_eq!(final_speed[0].multiplier, 4);
        let rolling_average = hooks.drain_camera_mod_rolling_average_requests();
        assert_eq!(rolling_average.len(), 1);
        assert_eq!(rolling_average[0].frames, 6);
        let visual_speed = hooks.drain_visual_speed_multiplier_requests();
        assert_eq!(visual_speed.len(), 1);
        assert_eq!(visual_speed[0].multiplier, 3);
        let script_freeze = hooks.drain_script_freeze_time_requests();
        assert_eq!(script_freeze, vec![true, false]);
        let fps_limit = hooks.drain_set_fps_limit_requests();
        assert_eq!(fps_limit.len(), 1);
        assert_eq!(fps_limit[0].fps, 120);
    }

    #[test]
    fn handler_forwards_oversize_terrain_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .oversize_terrain(2)
            .expect("oversize terrain request should succeed");
        handler
            .oversize_terrain(0)
            .expect("reset oversize terrain request should succeed");

        let requests = hooks.drain_oversize_terrain_requests();
        assert_eq!(requests, vec![2, 0]);
    }

    #[test]
    fn handler_forwards_border_shroud_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_border_shroud_level(32)
            .expect("set_border_shroud_level should succeed");
        handler
            .set_border_shroud_level(128)
            .expect("set_border_shroud_level should succeed");

        let requests = hooks.drain_border_shroud_levels();
        assert_eq!(requests, vec![32, 128]);
    }

    #[test]
    fn handler_forwards_military_caption_duration_as_milliseconds() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .military_caption("SCRIPT:Briefing", 2500)
            .expect("military caption request should succeed");

        let captions = hooks.drain_military_captions();
        assert_eq!(captions.len(), 1);
        assert_eq!(captions[0].text, "SCRIPT:Briefing");
        assert_eq!(captions[0].duration_ms, 2500);
    }

    #[test]
    fn speech_subtitle_label_matches_cpp_dialogevent_shape() {
        assert_eq!(
            speech_subtitle_label("USA01Intro"),
            "DIALOGEVENT:USA01IntroSubtitle"
        );
    }

    #[test]
    fn speech_subtitle_requires_displayable_localized_text() {
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |label| {
                assert_eq!(label, "DIALOGEVENT:BriefingSubtitle");
                Some("Commander online".to_string())
            }),
            Some("DIALOGEVENT:BriefingSubtitle".to_string())
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| None),
            None
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| Some(String::new())),
            None
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| Some("* hidden".to_string())),
            None
        );
    }

    #[test]
    fn speech_frames_from_length_ms_truncates_like_cpp() {
        // C++ REAL_TO_UNSIGNEDINT(audioLength / MSEC_PER_LOGICFRAME_REAL).
        assert_eq!(speech_frames_from_length_ms(0.0), 0);
        assert_eq!(speech_frames_from_length_ms(5_000.0), 150);
        assert_eq!(speech_frames_from_length_ms(33.3), 0);
        assert_eq!(speech_frames_from_length_ms(33.34), 1);
        assert_eq!(speech_frames_from_length_ms(1_000.0), 30);
    }

    #[test]
    fn has_finished_speech_uses_audio_length_not_one_frame() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        hooks.note_logic_frame(10);
        assert!(
            !hooks.is_speech_complete("", false),
            "empty speech name is not complete"
        );

        // Seed a 5s VO completion (150 frames) as TheAudio would.
        hooks.with_completion_tracking_for_test(|state| {
            state.speech_complete_frame.insert(
                "Briefing".to_string(),
                10 + speech_frames_from_length_ms(5_000.0),
            );
        });
        assert!(
            !hooks.is_speech_complete("Briefing", false),
            "HAS_FINISHED_SPEECH must stay false one frame after a 5s line"
        );
        hooks.note_logic_frame(11);
        assert!(
            !hooks.is_speech_complete("Briefing", false),
            "HAS_FINISHED_SPEECH must stay false until TheAudio length elapses"
        );
        hooks.note_logic_frame(159);
        assert!(!hooks.is_speech_complete("Briefing", false));
        hooks.note_logic_frame(160);
        assert!(hooks.is_speech_complete("Briefing", true));
        assert!(
            hooks.with_completion_tracking_for_test(|state| {
                state.speech_complete_frame.get("Briefing").is_none()
            }),
            "flush removes the completed speech tracker"
        );
    }

    #[test]
    fn has_finished_audio_uses_the_audio_length_not_one_frame() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        hooks.note_logic_frame(10);
        assert!(
            !hooks.is_audio_complete("", false),
            "empty audio name is not complete"
        );

        // Seed a 5s SFX completion (150 frames) as leftover TheAudio would.
        hooks.with_completion_tracking_for_test(|state| {
            state.audio_complete_frame.insert(
                "Boom".to_string(),
                10 + speech_frames_from_length_ms(5_000.0),
            );
        });
        assert!(
            !hooks.is_audio_complete("Boom", false),
            "HAS_FINISHED_AUDIO must stay false one frame after a 5s SFX"
        );
        hooks.note_logic_frame(11);
        assert!(
            !hooks.is_audio_complete("Boom", false),
            "HAS_FINISHED_AUDIO must stay false until leftover TheAudio length elapses"
        );
        hooks.note_logic_frame(159);
        assert!(!hooks.is_audio_complete("Boom", false));
        hooks.note_logic_frame(160);
        assert!(hooks.is_audio_complete("Boom", true));
        assert!(
            hooks.with_completion_tracking_for_test(|state| {
                state.audio_complete_frame.get("Boom").is_none()
            }),
            "flush removes the completed audio tracker"
        );
    }

    #[test]
    fn has_finished_video_waits_leftover_list_unknown_names_false() {
        // Standalone video bookkeeping needs explicit engine startup; hook
        // construction must not install it as a hidden side effect.
        initialize_script_engine().unwrap();
        let hooks = MissionScriptHooks::new().expect("hooks");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "unknown / never-finished names stay false"
        );

        handler
            .movie_play_fullscreen("IntroMovie")
            .expect("movie play should queue");
        hooks.note_logic_frame(1);
        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "HAS_FINISHED_VIDEO must not complete one frame after play"
        );

        gamelogic::helpers::TheScriptEngine::notify_of_completed_video("IntroMovie");
        assert!(
            handler.is_video_complete("IntroMovie", true),
            "leftover m_completedVideo membership is true"
        );
        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "flush removes the leftover completed-video entry"
        );
    }

    #[test]
    fn handler_forwards_radar_force_updates() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_radar_forced(true)
            .expect("radar force request should succeed");
        handler
            .set_radar_forced(false)
            .expect("radar revert request should succeed");

        assert_eq!(hooks.drain_radar_forced_updates(), vec![true, false]);
        assert!(hooks.drain_radar_forced_updates().is_empty());
    }

    #[test]
    fn handler_forwards_radar_event_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .create_radar_event(10.0, 20.0, 5.0, 3)
            .expect("radar event request should succeed");

        let requests = hooks.drain_radar_event_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, Vec3::new(10.0, 20.0, 5.0));
        assert_eq!(requests[0].event_type, 3);
    }

    #[test]
    fn zoom_camera_preserves_script_ease_parameters() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .zoom_camera(0.65, 4.0, 1.5, 1.0)
            .expect("zoom action should succeed");

        let zoom = hooks.drain_camera_zoom_requests();
        assert_eq!(zoom.len(), 1);
        assert!((zoom[0].zoom - 0.65).abs() < f32::EPSILON);
        assert!((zoom[0].duration_seconds - 4.0).abs() < f32::EPSILON);
        assert!((zoom[0].ease_in_seconds - 1.5).abs() < f32::EPSILON);
        assert!((zoom[0].ease_out_seconds - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn start_new_map_fade_calls_leftover_new_map() {
        let src = concat!(
            include_str!("mod.rs"),
            include_str!("script_requests.rs"),
            include_str!("script_engine.rs"),
            include_str!("script_hooks.rs"),
            include_str!("script_actions.rs"),
            include_str!("tests.rs"),
        );
        assert!(src.contains("pub fn start_new_map_fade"));
        assert!(src.contains("engine.new_map()"));
        let load = include_str!("../world_scripts/add_object_selection.rs");
        assert!(load.contains("engine.new_map()"));
        assert!(load.contains("FADE_MULTIPLY"));
    }

    #[test]
    fn handler_forwards_setup_and_look_toward_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .setup_camera(10.0, 20.0, 30.0, 0.7, 1.1, 40.0, 50.0, 60.0)
            .expect("setup camera should succeed");
        handler
            .camera_look_toward_object(42, 3.0, 1.5, 0.4, 0.6)
            .expect("look toward object should succeed");
        handler
            .camera_look_toward_waypoint(100.0, 200.0, 5.0, 2.0, 0.5, 0.25, true)
            .expect("look toward waypoint should succeed");
        handler
            .camera_mod_look_toward(70.0, 80.0, 90.0)
            .expect("camera mod look toward should succeed");
        handler
            .camera_mod_final_look_toward(15.0, 25.0, 35.0)
            .expect("camera mod final look toward should succeed");
        handler
            .move_camera_to_selection()
            .expect("move camera to selection should succeed");
        handler
            .camera_move_home()
            .expect("camera move home should succeed");
        handler
            .camera_set_default(0.75, 12.0, 1.8)
            .expect("camera set default should succeed");
        handler
            .camera_enable_slave_mode("CineCameraRig", "CameraBone")
            .expect("camera enable slave mode should succeed");
        handler
            .camera_disable_slave_mode()
            .expect("camera disable slave mode should succeed");
        handler
            .screen_shake(3)
            .expect("screen shake should succeed");
        handler
            .camera_add_shaker_at(5.0, 6.0, 7.0, 8.5, 2.5, 90.0)
            .expect("camera add shaker should succeed");
        handler
            .camera_follow_object(77, true)
            .expect("camera follow should succeed");
        handler
            .stop_camera_follow()
            .expect("camera stop follow should succeed");

        let setup = hooks.drain_camera_setup_requests();
        assert_eq!(setup.len(), 1);
        assert_eq!(setup[0].position, Vec3::new(10.0, 30.0, 20.0));
        assert!((setup[0].zoom - 0.7).abs() < f32::EPSILON);
        assert!((setup[0].pitch - 1.1).abs() < f32::EPSILON);
        assert_eq!(setup[0].look_toward, Vec3::new(40.0, 60.0, 50.0));

        let object = hooks.drain_camera_look_toward_object_requests();
        assert_eq!(object.len(), 1);
        assert_eq!(object[0].object_id, 42);
        assert!((object[0].duration_seconds - 3.0).abs() < f32::EPSILON);
        assert!((object[0].hold_seconds - 1.5).abs() < f32::EPSILON);
        assert!((object[0].ease_in_seconds - 0.4).abs() < f32::EPSILON);
        assert!((object[0].ease_out_seconds - 0.6).abs() < f32::EPSILON);

        let waypoint = hooks.drain_camera_look_toward_waypoint_requests();
        assert_eq!(waypoint.len(), 1);
        assert_eq!(waypoint[0].position, Vec3::new(100.0, 5.0, 200.0));
        assert!((waypoint[0].duration_seconds - 2.0).abs() < f32::EPSILON);
        assert!((waypoint[0].ease_in_seconds - 0.5).abs() < f32::EPSILON);
        assert!((waypoint[0].ease_out_seconds - 0.25).abs() < f32::EPSILON);
        assert!(waypoint[0].reverse_rotation);

        let mod_look = hooks.drain_camera_mod_look_toward_requests();
        assert_eq!(mod_look.len(), 1);
        assert_eq!(mod_look[0].position, Vec3::new(70.0, 90.0, 80.0));

        let mod_final_look = hooks.drain_camera_mod_final_look_toward_requests();
        assert_eq!(mod_final_look.len(), 1);
        assert_eq!(mod_final_look[0].position, Vec3::new(15.0, 35.0, 25.0));

        let move_to_selection = hooks.drain_camera_move_to_selection_requests();
        assert_eq!(move_to_selection.len(), 1);

        let move_home = hooks.drain_camera_move_home_requests();
        assert_eq!(move_home.len(), 1);

        let set_default = hooks.drain_camera_set_default_requests();
        assert_eq!(set_default.len(), 1);
        assert!((set_default[0].pitch - 0.75).abs() < f32::EPSILON);
        assert!((set_default[0].angle - 12.0).abs() < f32::EPSILON);
        assert!((set_default[0].max_height - 1.8).abs() < f32::EPSILON);

        let slave_enable = hooks.drain_camera_slave_mode_enable_requests();
        assert_eq!(slave_enable.len(), 1);
        assert_eq!(slave_enable[0].thing_template_name, "CineCameraRig");
        assert_eq!(slave_enable[0].bone_name, "CameraBone");
        let slave_disable = hooks.drain_camera_slave_mode_disable_requests();
        assert_eq!(slave_disable.len(), 1);

        let screen_shakes = hooks.drain_screen_shake_requests();
        assert_eq!(screen_shakes.len(), 1);
        assert_eq!(screen_shakes[0].intensity, 3);

        let shakers = hooks.drain_camera_add_shaker_requests();
        assert_eq!(shakers.len(), 1);
        assert_eq!(shakers[0].position, Vec3::new(5.0, 7.0, 6.0));
        assert!((shakers[0].amplitude - 8.5).abs() < f32::EPSILON);
        assert!((shakers[0].duration_seconds - 2.5).abs() < f32::EPSILON);
        assert!((shakers[0].radius - 90.0).abs() < f32::EPSILON);

        let follows = hooks.drain_camera_follows();
        assert_eq!(follows.len(), 2);
        assert_eq!(follows[0].object_id, 77);
        assert!(follows[0].snap_to_unit);
        assert_eq!(follows[1].object_id, 0);
        assert!(!follows[1].snap_to_unit);
    }

    #[test]
    fn music_track_completion_is_not_immediate_and_respects_flush() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "unknown / unplayed tracks stay false (C++ Miles loop count)"
        );

        handler
            .music_set_track("TrackA", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "track should not complete on the next frame without Miles loop count"
        );

        hooks.update(1).expect("frame advance should succeed");
        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "one logic frame is not a Miles loop completion"
        );
    }

    #[test]
    fn stop_music_does_not_fail_open_music_complete() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .music_set_track("TrackB", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackB", 1),
            "newly started track should be incomplete before stop"
        );

        handler.stop_music().expect("stop music should succeed");
        assert!(
            !handler.has_music_track_completed("TrackB", 1),
            "stop music does not mark hasMusicTrackCompleted; unplayed stays false"
        );
    }

    #[test]
    fn music_set_track_queues_named_track_on_the_audio() {
        // C++ ScriptActions::doMusicTrackChange → TheAudio->addAudioEvent(track).
        // Live GAME_SHELL uses MissionScriptActionHandler, which previously only
        // noted the name and never queued AR_Play.
        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        let before = {
            let guard = manager.lock().expect("THE_AUDIO lock");
            (
                guard.pending_play_request_count(),
                guard.get_music_track_name(),
            )
        };

        if let Ok(mut guard) = manager.lock() {
            if guard.find_audio_event_info("ShellMapMusic").is_none() {
                guard.register_audio_event_info(game_engine::common::audio::AudioEventInfo {
                    sound_type: game_engine::common::audio::AudioType::Music,
                    control: 0,
                    audio_name: "ShellMapMusic".to_string(),
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
                    sound_type_field: game_engine::common::audio::AudioType::Music,
                    type_field: 0,
                    priority: game_engine::common::audio::AudioPriority::Normal,
                    min_distance: 25.0,
                    max_distance: 1000.0,
                    low_pass_freq: 1.0,
                    is_level_specific: false,
                });
            }
        }

        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks);
        handler
            .music_set_track("ShellMapMusic", false, true)
            .expect("MUSIC_SET_TRACK must succeed");

        let after = {
            let guard = manager.lock().expect("THE_AUDIO lock");
            (
                guard.pending_play_request_count(),
                guard.get_music_track_name(),
            )
        };
        assert!(
            after.0 > before.0 || after.1 == "ShellMapMusic",
            "MUSIC_SET_TRACK must queue TheAudio AR_Play for the script track (before={before:?}, after={after:?})"
        );
        assert_eq!(
            after.1, "ShellMapMusic",
            "TheAudio music name must be the script track, not a leftover"
        );
    }

    #[test]
    fn music_set_track_does_not_broadcast_track_name() {
        // C++ ScriptActions::doMusicTrackChange has no TheInGameUI->message.
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());
        handler
            .music_set_track("ShellMapMusic", false, true)
            .expect("MUSIC_SET_TRACK must succeed");
        let messages = hooks.drain_messages();
        assert!(
            messages.iter().all(|m| !m.contains("Music track:")),
            "MUSIC_SET_TRACK must not broadcast Music track: name: {messages:?}"
        );
    }

    #[test]
    fn handler_forwards_weather_visibility_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_weather_visible(false)
            .expect("set weather visible should succeed");
        handler
            .set_weather_visible(true)
            .expect("set weather visible should succeed");

        assert_eq!(hooks.drain_weather_visibility_updates(), vec![false, true]);
    }

    #[test]
    fn popup_generation_remains_unique_across_replaced_hook_instances() {
        let popup = |message: &str| ScriptPopupMessageRequest {
            message: message.to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: false,
            pause_music: false,
            popup_generation: 0,
        };

        // A map load replaces GameLogic and its MissionScriptHooks. The
        // live-only token must therefore not restart at one per hook object.
        let old_world = MissionScriptHooks::new().expect("old world hooks");
        old_world.push_popup_message(popup("old popup"));
        let old_generation = old_world.drain_popup_message_requests()[0].popup_generation;

        let replacement_world = MissionScriptHooks::new().expect("replacement world hooks");
        replacement_world.push_popup_message(popup("replacement popup"));
        let replacement_generation =
            replacement_world.drain_popup_message_requests()[0].popup_generation;

        assert_ne!(old_generation, 0);
        assert_ne!(replacement_generation, 0);
        assert_ne!(
            old_generation, replacement_generation,
            "a stale old-world acknowledgement must not ABA-match the replacement world"
        );
    }

    #[test]
    fn handler_forwards_popup_guardband_motion_blur_and_ui_display_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .popup_message("Incoming transmission", 35, 55, 420, true, false)
            .expect("popup message should succeed");
        handler
            .resize_view_guardband(1.25, 0.75)
            .expect("resize view guardband should succeed");
        handler
            .set_camera_bw_mode(true, 24)
            .expect("set camera bw mode should succeed");
        handler
            .set_skybox_enabled(false)
            .expect("set skybox enabled should succeed");
        handler
            .camera_motion_blur(false, true)
            .expect("camera motion blur should succeed");
        handler
            .camera_motion_blur_jump(10.0, 20.0, 30.0, false)
            .expect("camera motion blur jump should succeed");
        handler
            .camera_motion_blur_follow(8)
            .expect("camera motion blur follow should succeed");
        handler
            .camera_motion_blur_end_follow()
            .expect("camera motion blur end follow should succeed");
        handler
            .cameo_flash("Command_ConstructChinaBarracks", 7)
            .expect("cameo flash should succeed");
        handler
            .add_named_timer("TimerA", "Launch Window", true)
            .expect("add named timer should succeed");
        handler
            .remove_named_timer("TimerA")
            .expect("remove named timer should succeed");
        handler
            .show_named_timer_display(true)
            .expect("show named timer display should succeed");
        handler
            .set_superweapon_display_enabled_by_script(false)
            .expect("set superweapon display enabled should succeed");
        handler
            .hide_object_superweapon_display_by_script(77)
            .expect("hide object superweapon display should succeed");
        handler
            .show_object_superweapon_display_by_script(77)
            .expect("show object superweapon display should succeed");

        let popups = hooks.drain_popup_message_requests();
        assert_eq!(popups.len(), 1);
        assert_eq!(popups[0].message, "Incoming transmission");
        assert_eq!(popups[0].x_percent, 35);
        assert_eq!(popups[0].y_percent, 55);
        assert_eq!(popups[0].width, 420);
        assert!(popups[0].pause);
        assert!(!popups[0].pause_music);

        let guardbands = hooks.drain_view_guardband_requests();
        assert_eq!(
            guardbands,
            vec![ViewGuardbandRequest {
                x_bias: 1.25,
                y_bias: 0.75
            }]
        );

        let bw = hooks.drain_camera_bw_mode_requests();
        assert_eq!(
            bw,
            vec![CameraBwModeRequest {
                enabled: true,
                frames: 24
            }]
        );

        assert_eq!(hooks.drain_skybox_enabled_updates(), vec![false]);

        let blur = hooks.drain_camera_motion_blur_requests();
        assert_eq!(blur.len(), 4);
        assert_eq!(
            blur[0],
            CameraMotionBlurRequest::Basic {
                zoom_in: false,
                saturate: true
            }
        );
        assert_eq!(
            blur[1],
            CameraMotionBlurRequest::Jump {
                position: Vec3::new(10.0, 30.0, 20.0),
                saturate: false
            }
        );
        assert_eq!(blur[2], CameraMotionBlurRequest::Follow { amount: 8 });
        assert_eq!(blur[3], CameraMotionBlurRequest::EndFollow);

        let cameo = hooks.drain_cameo_flash_requests();
        assert_eq!(cameo.len(), 1);
        assert_eq!(
            cameo[0].command_button_name,
            "Command_ConstructChinaBarracks"
        );
        assert_eq!(cameo[0].flash_count, 7);

        let timers = hooks.drain_named_timer_mutations();
        assert_eq!(
            timers,
            vec![
                NamedTimerMutation::Add {
                    name: "TimerA".to_string(),
                    text: "Launch Window".to_string(),
                    countdown: true
                },
                NamedTimerMutation::Remove {
                    name: "TimerA".to_string()
                }
            ]
        );
        assert_eq!(hooks.drain_named_timer_display_updates(), vec![true]);
        assert_eq!(
            hooks.drain_superweapon_display_enabled_updates(),
            vec![false]
        );
        assert_eq!(
            hooks.drain_superweapon_object_display_mutations(),
            vec![
                SuperweaponObjectDisplayMutation::Hide { object_id: 77 },
                SuperweaponObjectDisplayMutation::Show { object_id: 77 }
            ]
        );
    }

    fn win_lose_layout_is_open(root_name: &str, child_name: &str) -> bool {
        game_client::gui::with_window_manager_ref(|wm| {
            wm.find_window_by_name(root_name).is_some()
                || wm.find_window_by_name(child_name).is_some()
        })
    }

    #[test]
    fn host_create_win_lose_window_loads_victorious_and_defeat_layouts() {
        // C++ ScriptActions.cpp:204/228 TheWindowManager->winCreateFromScript
        // ("Menus/Victorious.wnd" / "Menus/Defeat.wnd"). Live host handler must
        // not stay the ScriptActionHandler default no-op.
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks);
        let _ = handler.destroy_win_lose_window();

        handler
            .create_win_lose_window("Menus/Victorious.wnd")
            .expect("create Victorious.wnd");
        assert!(
            win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "host MissionScriptActionHandler must load Menus/Victorious.wnd"
        );

        handler
            .create_win_lose_window("Menus/Defeat.wnd")
            .expect("create Defeat.wnd");
        assert!(
            win_lose_layout_is_open("Defeat.wnd:Defeat", "Defeat.wnd:DefeatImage"),
            "host MissionScriptActionHandler must load Menus/Defeat.wnd"
        );
        assert!(
            !win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "creating Defeat.wnd must destroy the prior Victorious.wnd"
        );

        handler
            .destroy_win_lose_window()
            .expect("destroy Defeat.wnd");
        assert!(
            !win_lose_layout_is_open("Defeat.wnd:Defeat", "Defeat.wnd:DefeatImage"),
            "destroy_win_lose_window must remove the tracked message window"
        );
    }

    #[test]
    fn live_script_engine_victory_opens_victorious_wnd_via_host_handler() {
        // C++ ScriptActions::doVictory ScriptActions.cpp:191-209.
        initialize_script_engine().expect("script engine should initialize");
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler: Arc<dyn ScriptActionHandler> =
            Arc::new(MissionScriptActionHandler::new(hooks));
        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_action_handler(Some(Arc::clone(&handler)));
                engine.close_windows(false);
                engine.create_win_lose_window("Menus/Victorious.wnd");
                assert_eq!(
                    engine.current_win_lose_window().as_deref(),
                    Some("Menus/Victorious.wnd")
                );
            }
        }
        assert!(
            win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "live ScriptEngine + host handler must materialise Victorious.wnd"
        );
        let _ = handler.destroy_win_lose_window();
    }

    #[test]
    fn live_quick_victory_starts_timer_then_posts_clear_game_data() {
        // C++ ScriptActions.cpp:169-176 doQuickVictory → startQuickEndGameTimer.
        // ScriptEngine.cpp:5514-5518 expiry appends MSG_CLEAR_GAME_DATA.
        use game_engine::common::message_stream::{GameMessageType, get_message_stream};
        use gamelogic::scripting::core::ScriptAction;
        use gamelogic::scripting::evaluator::ScriptEvaluator;

        initialize_script_engine().expect("script engine should initialize");
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler: Arc<dyn ScriptActionHandler> =
            Arc::new(MissionScriptActionHandler::new(hooks));
        {
            let stream = get_message_stream();
            let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
            stream.clear_messages();
        }
        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_action_handler(Some(Arc::clone(&handler)));
                engine.close_windows(false);
                engine.set_campaign_victorious(false);
            }
        }

        let evaluator = ScriptEvaluator::new(get_script_engine());
        evaluator
            .execute_action(&ScriptAction::new(ScriptActionType::Quickvictory))
            .expect("QuickVictory should execute");

        {
            let engine = get_script_engine();
            let guard = engine.read().expect("script engine");
            let engine = guard.as_ref().expect("initialized");
            assert!(
                engine.is_game_ending(),
                "C++ startQuickEndGameTimer must arm m_endGameTimer"
            );
            assert!(
                engine.is_campaign_victorious(),
                "C++ ScriptActions.cpp:175 SetVictorious(TRUE)"
            );
        }

        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine
                    .update()
                    .expect("one-frame quick-end timer should expire");
            }
        }

        let stream = get_message_stream();
        let stream = stream.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            stream.contains_message_of_type(&GameMessageType::ClearGameData),
            "timer expiry must append MSG_CLEAR_GAME_DATA (ScriptEngine.cpp:5518)"
        );
        let _ = handler.destroy_win_lose_window();
    }

    #[test]
    fn reset_camera_to_forwards_scripted_ease() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        let handler = MissionScriptActionHandler::new(hooks.clone());
        handler
            .reset_camera_to(10.0, 20.0, 3.0, 2.0, 0.4, 0.6)
            .expect("reset");
        let resets = hooks.drain_camera_resets();
        assert_eq!(resets.len(), 1);
        assert_eq!(resets[0].duration_seconds, 2.0);
        assert_eq!(resets[0].ease_in_seconds, 0.4);
        assert_eq!(resets[0].ease_out_seconds, 0.6);
        assert_eq!(resets[0].position, Vec3::new(10.0, 3.0, 20.0));
    }
}
