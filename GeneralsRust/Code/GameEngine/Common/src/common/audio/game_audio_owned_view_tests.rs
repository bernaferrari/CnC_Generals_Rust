use super::*;
struct ForeignView;
impl AudioViewResolver for ForeignView {
    fn get_tactical_view_position(&self) -> Coord3D {
        panic!("foreign camera read")
    }
    fn get_tactical_view_angle(&self) -> Real {
        panic!("foreign heading read")
    }
    fn get_3d_camera_position(&self) -> Coord3D {
        panic!("foreign camera read")
    }
    fn get_ground_height(&self, _: Real, _: Real) -> Real {
        panic!("foreign terrain read")
    }
}
#[test]
fn explicit_audio_view_uses_receiving_camera_and_terrain_not_foreign_resolver() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "explicit_audio_view_uses_receiving_camera_and_terrain_not_foreign_resolver",
    ) {
        return;
    }
    assert!(register_audio_view_resolver(Arc::new(ForeignView)));
    let mut owner = AudioManager::new();
    owner.audio_settings.microphone_desired_height_above_terrain = 100.;
    owner
        .audio_settings
        .microphone_max_percentage_between_ground_and_camera = 0.5;
    owner.update_with_view(
        AudioViewObservation {
            ground_position: Coord3D {
                x: 10.,
                y: 20.,
                z: 30.,
            },
            heading: 0.,
            camera_position: Coord3D {
                x: 110.,
                y: 220.,
                z: 430.,
            },
            ground_height: 17.,
        },
        &mut |_| true,
    );
    let listener = owner.get_listener_position();
    assert_eq!((listener.x, listener.y, listener.z), (35., 70., 117.));
    let mut peer = AudioManager::new();
    peer.update_with_view(
        AudioViewObservation {
            ground_position: Coord3D {
                x: -10.,
                y: -20.,
                z: 0.,
            },
            heading: 1.,
            camera_position: Coord3D {
                x: -10.,
                y: -20.,
                z: 10.,
            },
            ground_height: 3.,
        },
        &mut |_| true,
    );
    let listener = owner.get_listener_position();
    assert_eq!((listener.x, listener.y, listener.z), (35., 70., 117.));
}
struct ExplicitPlayback(std::sync::atomic::AtomicUsize);
impl SoundPlaybackHook for ExplicitPlayback {
    fn play(&self, _: &AudioEventRts) -> Result<(), String> {
        panic!("legacy playback selected")
    }
    fn play_with_visibility(&self, _: &AudioEventRts, visible: bool) -> Result<(), String> {
        assert!(!visible);
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn stop(&self, _: AudioHandle) {}
    fn pause(&self, _: AudioHandle) {}
    fn resume(&self, _: AudioHandle) {}
    fn is_playing(&self, _: AudioHandle) -> bool {
        false
    }
}
#[test]
fn explicit_visibility_reaches_actual_request_and_loop_replay_without_foreign_view() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "explicit_visibility_reaches_actual_request_and_loop_replay_without_foreign_view",
    ) {
        return;
    }
    assert!(register_audio_view_resolver(Arc::new(ForeignView)));
    let hook = Arc::new(ExplicitPlayback(std::sync::atomic::AtomicUsize::new(0)));
    assert!(register_sound_playback_hook(hook.clone()));
    let mut manager = AudioManager::new();
    let info = Arc::new(AudioEventInfo {
        audio_name: "OwnedVisibility".into(),
        sound_type: AudioType::SoundEffect,
        sound_type_field: AudioType::SoundEffect,
        type_field: ST_WORLD,
        control: AC_LOOP,
        low_pass_freq: 0.5,
        loop_count: 3,
        sounds: vec!["owned.wav".into()],
        volume: 1.,
        pitch_shift_min: 1.,
        pitch_shift_max: 1.,
        ..Default::default()
    });
    let mut event = AudioEventRts::new();
    event.set_event_name("OwnedVisibility".into());
    event.set_audio_event_info(info);
    event.set_playing_handle(AHSV_FIRST_HANDLE + 17);
    event.set_position(&Coord3D {
        x: 2.,
        y: 3.,
        z: 4.,
    });
    event.set_volume(1.);
    event.set_volume_shift(1.);
    event.set_loop_count(3);
    event.set_next_play_portion(PortionToPlay::Sound);
    manager.append_audio_request(AudioRequest::new_with_event(
        RequestType::Play,
        event.clone(),
    ));
    let observation = AudioViewObservation {
        ground_position: Coord3D::ZERO,
        heading: 0.,
        camera_position: Coord3D {
            x: 0.,
            y: 0.,
            z: 100.,
        },
        ground_height: 0.,
    };
    let mut queries = 0;
    manager.update_with_view(observation, &mut |position| {
        assert_eq!((position.x, position.y, position.z), (2., 3., 4.));
        queries += 1;
        false
    });
    assert!(
        hook.0.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "actual queued play reached explicit hook"
    );
    let before = hook.0.load(std::sync::atomic::Ordering::SeqCst);
    manager.insert_playing_event_for_test(event);
    manager.update_with_view(observation, &mut |_| {
        queries += 1;
        false
    });
    assert!(
        hook.0.load(std::sync::atomic::Ordering::SeqCst) > before,
        "actual completion loop replay reached explicit hook"
    );
    assert_eq!(queries, hook.0.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn receiving_listener_matches_extracted_cpp_component_records() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "receiving_listener_matches_extracted_cpp_component_records",
    ) {
        return;
    }
    let records: serde_json::Value =
        serde_json::from_str(include_str!("audio_listener_cpp_records.json")).unwrap();
    let records = records.as_array().unwrap();
    assert_eq!(records.len(), 8);
    let value = |record: &serde_json::Value, name: &str, index: usize| {
        record[name][index].as_f64().unwrap() as f32
    };
    let coord = |record: &serde_json::Value, name: &str| Coord3D {
        x: value(record, name, 0),
        y: value(record, name, 1),
        z: value(record, name, 2),
    };
    fn close(actual: f32, expected: f32, case: usize, field: &str) {
        assert!(
            (actual - expected).abs() <= 2e-5 * expected.abs().max(1.),
            "case{case} {field}: {actual} versus {expected}"
        );
    }
    for (case, record) in records.iter().enumerate() {
        let mut manager = AudioManager::new();
        manager
            .audio_settings
            .microphone_desired_height_above_terrain = value(record, "settings", 0);
        manager
            .audio_settings
            .microphone_max_percentage_between_ground_and_camera = value(record, "settings", 1);
        manager.audio_settings.zoom_sound_volume_percentage_amount = value(record, "settings", 2);
        manager.audio_settings.zoom_min_distance = value(record, "settings", 3);
        manager.audio_settings.zoom_max_distance = value(record, "settings", 4);
        manager.update_with_view(
            AudioViewObservation {
                ground_position: coord(record, "ground"),
                heading: record["heading"].as_f64().unwrap() as f32,
                camera_position: coord(record, "eye"),
                ground_height: record["terrain"].as_f64().unwrap() as f32,
            },
            &mut |_| panic!("empty component update must not query play visibility"),
        );
        for (name, actual) in [
            ("listener", manager.get_listener_position()),
            ("orientation", manager.get_listener_orientation()),
        ] {
            for (index, actual) in [actual.x, actual.y, actual.z].into_iter().enumerate() {
                close(actual, value(record, name, index), case, name);
            }
        }
        close(
            manager.get_zoom_volume(),
            record["zoom"].as_f64().unwrap() as f32,
            case,
            "zoom",
        );
    }
}
