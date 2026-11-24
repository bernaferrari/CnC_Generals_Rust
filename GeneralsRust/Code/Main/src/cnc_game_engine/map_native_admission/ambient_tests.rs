//! Exercise the actual object Dict and Drawable constructor on a mounted CkMp map.
use super::staged_native_fixture_gpu;
use crate::cnc_game_engine::map_native_admission::NativeMapCandidate;
use crate::cnc_game_engine::native_audio_effects::NativeAudioService;
use crate::cnc_game_engine::native_view_effects::receiving_native_drawable_policy_for;
use crate::game_logic::{GameLogic, GameMode, NativeMapAdmission};
use crate::subsystem_manager::{AudioManagerSubsystem, SubsystemManager};
use game_client::core::game_client::NativeParticleRetirementService;
use game_engine::common::audio::{
    AC_LOOP, AudioAffect, AudioEventInfo, AudioManager, AudioPriority, ST_GLOBAL,
};
use game_engine::common::{dict::Dict, system::DataChunkOutput, well_known_keys::*};
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use std::sync::{Arc, Mutex};

const AMBIENT_TEMPLATE: &str = "OwnedMapAmbientFixture";
const ONE_SHOT: &str = "OwnedMapAmbientOneShot";
const LOOP: &str = "OwnedMapAmbientLoop";

fn write_map(path: &std::path::Path, volume: f32) {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [32, 32, 0, 1024] {
        map.write_int(value);
    }
    for _ in 0..1024 {
        map.write_byte(40);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    for (index, (name, enabled, customize)) in [
        (ONE_SHOT, None, false),
        (ONE_SHOT, Some(true), false),
        (LOOP, Some(true), true),
        ("", Some(true), true),
    ]
    .into_iter()
    .enumerate()
    {
        map.open_data_chunk("Object", 3);
        for value in [20. + index as f32 * 30., 20., 0., 0.] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string(AMBIENT_TEMPLATE);
        let mut properties = Dict::new();
        properties.set_ascii_string(key_object_name(), &format!("AmbientPlacement{index}"));
        properties.set_ascii_string(key_object_sound_ambient(), name);
        if let Some(enabled) = enabled {
            properties.set_bool(key_object_sound_ambient_enabled(), enabled);
        }
        if customize {
            properties.set_bool(key_object_sound_ambient_customized(), true);
            properties.set_int(key_object_sound_ambient_loop_count(), 3);
            properties.set_real(key_object_sound_ambient_volume(), volume);
            properties.set_real(key_object_sound_ambient_min_volume(), 0.125);
            properties.set_real(key_object_sound_ambient_min_range(), 50.);
            properties.set_real(key_object_sound_ambient_max_range(), 225.);
            properties.set_int(key_object_sound_ambient_priority(), 3);
        }
        map.write_dict(&properties);
        map.close_data_chunk();
    }
    map.close_data_chunk();
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
}

fn set_script_sound(
    world: &mut GameLogic,
    attempt: &mut dyn NativeMapAdmission,
    actor: &str,
    kind: ScriptActionType,
) {
    assert!(world.host_object_id_by_script_name(actor).is_some());
    world
        .with_owned_script_engine(|engine, world| {
            let mut action = ScriptAction::new(kind);
            action
                .add_parameter(Parameter::with_string(ParameterType::Unit, actor.into()))
                .unwrap();
            world
                .execute_script_team_actions_with_effects_checked(
                    engine,
                    &action,
                    None,
                    &mut attempt.effects(),
                )
                .unwrap();
        })
        .unwrap();
}

#[test]
fn authored_map_ambient_uses_owned_drawable_overrides_and_one_shot_defaults() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_map_ambient_uses_owned_drawable_overrides_and_one_shot_defaults",
        || {
            let (_gpu, mut graphics, mut pipeline) = staged_native_fixture_gpu();
            let directory = tempfile::tempdir().unwrap();
            let device = Arc::new(Mutex::new(AudioManager::new()));
            {
                let mut mixer = device.lock().unwrap();
                mixer.init();
                for (name, control, count) in [(ONE_SHOT, 0, 1), (LOOP, AC_LOOP, 0)] {
                    mixer.register_audio_event_info(AudioEventInfo {
                        audio_name: name.into(),
                        sounds: vec!["map_ambient_fixture".into()],
                        type_field: ST_GLOBAL,
                        volume: 0.75,
                        control,
                        loop_count: count,
                        ..Default::default()
                    });
                }
            }
            let mut candidates = Vec::new();
            let mut observations = Vec::new();
            for (index, volume) in [0.25, 0.5].into_iter().enumerate() {
                let file = directory.path().join(format!("Ambient{index}.map"));
                write_map(&file, volume);
                let path = file.to_str().unwrap();
                let mut world = GameLogic::new();
                world
                    .admit_receiving_module_content(
                        crate::assets::ReceivingObjectContent::from_sources([(
                            "ambient-map-test.ini".to_owned(),
                            format!(
                                "Object {AMBIENT_TEMPLATE}\nKindOf = IMMOBILE\nSoundAmbient = {LOOP}\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n"
                            ),
                        )])
                        .unwrap(),
                    )
                    .unwrap();
                world.start_new_game(GameMode::SinglePlayer).unwrap();
                world.require_native_drawable_owner();
                let mut candidate = NativeMapCandidate::with_epoch(992);
                let mut subsystems = SubsystemManager::new();
                let mut receiving_audio = AudioManagerSubsystem::new();
                receiving_audio.bind_common_audio_device(Arc::clone(&device));
                let instance = receiving_audio.audio_instance().clone();
                subsystems.add_subsystem(receiving_audio);
                let mut retirement = NativeParticleRetirementService;
                let mut attempt = candidate
                    .attempt(
                        pipeline.borrow_native_draw_services(),
                        &mut graphics,
                        0,
                        receiving_native_drawable_policy_for(&world),
                        &mut retirement,
                        Some((Arc::clone(&device), &instance)),
                    )
                    .unwrap();
                assert!(
                    world
                        .load_map_with_definition_source_and_water_delivery_and_native_checked(
                            path,
                            path,
                            None,
                            Some(&mut attempt),
                        )
                        .unwrap()
                );
                assert_eq!(
                    world.host_objects().registered_keys().count(),
                    4,
                    "all four map placements were admitted"
                );
                let mixer = device.lock().unwrap();
                let events: Vec<_> = mixer
                    .pending_play_events()
                    .skip(2 * index)
                    .map(|event| {
                        (
                            event.get_drawable_id(),
                            event.get_audio_event_info().unwrap(),
                        )
                    })
                    .collect();
                assert_eq!(events.len(), 2, "only the explicitly enabled rows play");
                let (one_shot, finite) = {
                    let one_shot = events
                        .iter()
                        .find(|(_, info)| info.audio_name.ends_with(ONE_SHOT))
                        .unwrap();
                    let finite = events
                        .iter()
                        .find(|(_, info)| info.audio_name.ends_with(LOOP))
                        .unwrap();
                    (one_shot, finite)
                };
                assert_eq!(one_shot.1.control & AC_LOOP, 0);
                assert_eq!(one_shot.1.loop_count, 1);
                assert_eq!(finite.1.control & AC_LOOP, AC_LOOP);
                assert_eq!(finite.1.loop_count, 3);
                assert_eq!(finite.1.volume, volume);
                assert_eq!(finite.1.min_volume, 0.125);
                assert_eq!(finite.1.min_distance, 50.);
                assert_eq!(finite.1.max_distance, 225.);
                assert_eq!(finite.1.priority, AudioPriority::High);
                assert!(mixer.find_audio_event_info(&finite.1.audio_name).is_none());
                assert_eq!(mixer.find_audio_event_info(LOOP).unwrap().volume, 0.75);
                observations.push((finite.0, finite.1.audio_name.clone(), finite.1.volume));
                drop(mixer);
                assert!(
                    !world
                        .queued_audio_events
                        .iter()
                        .any(|event| { matches!(event.event_type.as_str(), ONE_SHOT | LOOP) })
                );
                if index == 1 {
                    world.initialize_receiving_named_cache_for_fixture();
                    set_script_sound(
                        &mut world,
                        &mut attempt,
                        "AmbientPlacement0",
                        ScriptActionType::EnableObjectSound,
                    );
                    set_script_sound(
                        &mut world,
                        &mut attempt,
                        "AmbientPlacement3",
                        ScriptActionType::EnableObjectSound,
                    );
                    let extra = {
                        let mixer = device.lock().unwrap();
                        mixer
                            .pending_play_events()
                            .skip(4)
                            .map(|event| {
                                (
                                    event.get_drawable_id(),
                                    event.get_audio_event_info().unwrap(),
                                )
                            })
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(
                        extra.len(),
                        1,
                        "a script can trigger a one-shot, while silence persists"
                    );
                    assert_ne!(extra[0].0, one_shot.0);
                    assert!(extra[0].1.audio_name.ends_with(ONE_SHOT));
                    assert_eq!(extra[0].1.control & AC_LOOP, 0);
                    let (previous, original_mix) = {
                        let mut mixer = device.lock().unwrap();
                        let ambient: Vec<_> = mixer
                            .pending_play_events()
                            .filter(|event| event.get_event_name() == finite.1.audio_name)
                            .map(|event| {
                                (
                                    event.get_playing_handle(),
                                    mixer.get_effective_volume(event),
                                )
                            })
                            .collect();
                        mixer.process_request_list();
                        (
                            ambient
                                .iter()
                                .map(|(handle, _)| *handle)
                                .collect::<Vec<_>>(),
                            ambient[1].1,
                        )
                    };
                    assert_eq!(previous.len(), 2);
                    set_script_sound(
                        &mut world,
                        &mut attempt,
                        "AmbientPlacement2",
                        ScriptActionType::DisableObjectSound,
                    );
                    {
                        let mixer = device.lock().unwrap();
                        assert!(mixer.contains_audio_handle(previous[0]));
                        assert!(!mixer.contains_audio_handle(previous[1]));
                    }
                    world
                        .queued_audio_events
                        .set_volume(0.5, AudioAffect::SoundEffects);
                    set_script_sound(
                        &mut world,
                        &mut attempt,
                        "AmbientPlacement2",
                        ScriptActionType::EnableObjectSound,
                    );
                    let mixer = device.lock().unwrap();
                    let restored = mixer.pending_play_events().next().unwrap();
                    assert_eq!(restored.get_event_name(), finite.1.audio_name);
                    assert_eq!(restored.get_audio_event_info().unwrap().loop_count, 3);
                    assert!(original_mix > 0.0);
                    assert!(
                        (mixer.get_effective_volume(restored) - original_mix * 0.5).abs() < 1e-6
                    );
                    let renewed = restored.get_playing_handle();
                    drop(mixer);
                    {
                        let mut mixer = device.lock().unwrap();
                        mixer.process_request_list();
                        assert!(mixer.contains_audio_handle(renewed));
                    }
                    let actor = world
                        .host_object_id_by_script_name("AmbientPlacement2")
                        .unwrap();
                    let mut receiving_audio =
                        NativeAudioService::receiving(&mut subsystems).unwrap();
                    let mut effects = attempt.effects();
                    effects.audio = Some(&mut receiving_audio);
                    let damage = world
                        .apply_owned_damage_with_effects(
                            actor,
                            80.,
                            None,
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                            None,
                            &crate::game_logic::object::DamageHitContext::default(),
                            &mut effects,
                        )
                        .unwrap()
                        .unwrap();
                    assert!(damage.destroyed());
                    let mixer = device.lock().unwrap();
                    assert!(
                        !mixer.contains_audio_handle(renewed),
                        "body transition retires its old ambient"
                    );
                    assert!(
                        mixer.contains_audio_handle(previous[0]),
                        "same-ID peer keeps its ambient"
                    );
                }
                drop(attempt);
                candidates.push(candidate);
            }
            assert_eq!(observations[0].0, observations[1].0);
            assert_eq!(observations[0].1, observations[1].1);
            assert_ne!(observations[0].2, observations[1].2);
        },
    );
}
