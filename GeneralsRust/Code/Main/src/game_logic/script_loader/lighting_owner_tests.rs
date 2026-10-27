//! Actual LightingData decoder -> receiving owner -> presentation observations.
use super::*;
use crate::game_logic::{GameLogic, lighting_rules::LightingContentRules};
use game_engine::common::ini::TimeOfDay;
fn payload(version: u16, tod: i32, base: f32) -> Vec<u8> {
    let mut bytes = tod.to_le_bytes().to_vec();
    for row in 0..4 {
        let mut write = |slot: usize| {
            let value = base + row as f32 + slot as f32 * 0.01;
            for f in [value, 0.2, 0.3, 0.4, 0.5, 0.6, 1.0, 2.0, 3.0] {
                bytes.extend_from_slice(&f.to_le_bytes());
            }
        };
        write(0);
        write(1);
        if version >= 2 {
            write(2);
            write(3);
        }
        if version >= 3 {
            write(4);
            write(5);
        }
    }
    bytes
}
fn parsed(version: u16, tod: i32, base: f32) -> MapMetadata {
    let mut metadata = MapMetadata::default();
    parse_lighting_payload_for_settings(version, &payload(version, tod, base), &mut metadata)
        .unwrap();
    metadata
}
#[test]
fn original_versions_reset_unstored_extra_lights_to_black_downward() {
    for version in 1..=3 {
        let mut world = GameLogic::new();
        let mut rules = LightingContentRules::default();
        rules.num_global_lights = 3;
        rules.objects[4][2].ambient.r = 99.0;
        rules.terrain[4][2].light_pos.z = 99.0;
        world.admit_lighting_rules_base(rules);
        let meta = parsed(version, 4, 0.1);
        world.apply_map_lighting(meta.lighting.as_ref().unwrap());
        let effective = world.lighting_rules();
        assert_eq!(effective.time_of_day, TimeOfDay::Night);
        assert!((effective.terrain[4][0].ambient.r - 3.1).abs() < 0.0001);
        assert!((effective.objects[4][0].ambient.r - 3.11).abs() < 0.0001);
        if version == 1 {
            assert_eq!(effective.objects[4][2].ambient.r, 0.0);
            assert_eq!(effective.objects[4][2].light_pos.z, -1.0);
        }
        if version < 3 {
            assert_eq!(effective.terrain[4][2].ambient.r, 0.0);
            assert_eq!(effective.terrain[4][2].light_pos.z, -1.0);
        }
        let env = crate::presentation_frame::PresentationWorldEnv::from_logic(&world);
        assert!(env.is_night);
        assert!((env.primary_object_lighting.unwrap().ambient[0] - 3.11).abs() < 0.0001);
    }
}
#[test]
fn candidate_decode_and_failed_lighting_never_change_another_receiving_scene() {
    let mut first = GameLogic::new();
    first.apply_map_lighting(parsed(3, 4, 0.1).lighting.as_ref().unwrap());
    let before = first.lighting_rules().objects[4][0].ambient.r;
    let mut second = GameLogic::new();
    let day = parsed(2, 2, 0.7);
    second.apply_map_lighting(day.lighting.as_ref().unwrap());
    let mut rejected = MapMetadata::default();
    assert!(
        parse_lighting_payload_for_settings(3, &payload(3, 1, 0.8)[..25], &mut rejected).is_err()
    );
    assert!(rejected.lighting.is_none());
    assert_eq!(first.lighting_rules().objects[4][0].ambient.r, before);
    assert!(crate::presentation_frame::PresentationWorldEnv::from_logic(&first).is_night);
    assert!(!crate::presentation_frame::PresentationWorldEnv::from_logic(&second).is_night);
    drop(second);
    assert_eq!(first.lighting_rules().time_of_day, TimeOfDay::Night);
}
#[test]
fn application_and_companion_time_of_day_are_explicit_ordered_and_reset_owned() {
    let base = LightingContentRules::default()
        .overlay_ini_text("GameData\nTimeOfDay = MORNING\nNumberGlobalLights = 3\nEnd")
        .unwrap();
    let mut first = GameLogic::new();
    first.admit_lighting_rules_base(base.clone());
    first
        .admit_lighting_companion_sources(&[
            "GameData\nTimeOfDay = EVENING\nEnd".into(),
            "GameData\nTimeOfDay = NIGHT\nEnd".into(),
        ])
        .unwrap();
    let peer = GameLogic::new();
    assert_eq!(first.lighting_rules().time_of_day, TimeOfDay::Night);
    assert_eq!(peer.lighting_rules().time_of_day, TimeOfDay::Afternoon);
    let retained = first.lighting_rules().time_of_day;
    assert!(
        first
            .admit_lighting_companion_sources(&["GameData\nTimeOfDay = UNKNOWN\nEnd".into()])
            .is_err()
    );
    assert_eq!(first.lighting_rules().time_of_day, retained);
    first.reset();
    assert_eq!(first.lighting_rules().time_of_day, TimeOfDay::Morning);
    assert_eq!(first.lighting_rules().num_global_lights, 3);
}
#[test]
fn versioned_lighting_continuation_preserves_effective_scene_and_rejects_before_mutation() {
    let mut source = GameLogic::new();
    let mut force_rules = LightingContentRules::default();
    force_rules.force_models_to_follow_time_of_day = false;
    force_rules.force_models_to_follow_weather = true;
    source.admit_lighting_rules_base(force_rules);
    source.apply_map_lighting(parsed(3, 4, 0.1).lighting.as_ref().unwrap());
    let bytes = bincode_legacy::serialize(&source.capture_lighting_continuation()).unwrap();
    let saved = bincode_legacy::deserialize::<
        crate::game_logic::lighting_rules::LightingContinuationV1,
    >(&bytes)
    .unwrap();
    let mut restored = GameLogic::new();
    restored.restore_lighting_continuation(&saved).unwrap();
    assert!(!restored.lighting_rules().force_models_to_follow_time_of_day);
    assert!(restored.lighting_rules().force_models_to_follow_weather);
    assert_eq!(restored.lighting_rules().time_of_day, TimeOfDay::Night);
    assert_eq!(
        restored.lighting_rules().objects[4][0].ambient.r,
        source.lighting_rules().objects[4][0].ambient.r
    );
    assert!(crate::presentation_frame::PresentationWorldEnv::from_logic(&restored).is_night);
    let mut malformed = saved.clone();
    malformed.version = 99;
    assert!(restored.restore_lighting_continuation(&malformed).is_err());
    assert_eq!(restored.lighting_rules().time_of_day, TimeOfDay::Night);
    restored.reset();
    assert_eq!(
        restored.lighting_rules().time_of_day,
        TimeOfDay::Afternoon,
        "saved effective values do not replace the admitted application baseline"
    );
}
#[test]
fn actual_map_admission_keeps_first_scene_lighting_when_second_map_is_loaded() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_map_admission_keeps_first_scene_lighting_when_second_map_is_loaded",
        || {
            use crate::game_logic::{GameLogic, GameMode};
            let directory = tempfile::tempdir().unwrap();
            let write_map = |name: &str, time: i32, shade: f32| {
                let mut output = game_engine::common::system::DataChunkOutput::new();
                output.open_data_chunk("HeightMapData", 3);
                for value in [8, 8, 0, 64] {
                    output.write_int(value);
                }
                for _ in 0..64 {
                    output.write_byte(40);
                }
                output.close_data_chunk();
                output.open_data_chunk("GlobalLighting", 1);
                output.write_int(time);
                for _ in 0..4 {
                    for value in [
                        shade,
                        0.2,
                        0.3,
                        0.4,
                        0.5,
                        0.6,
                        0.0,
                        0.0,
                        -1.0,
                        shade + 0.1,
                        0.2,
                        0.3,
                        0.4,
                        0.5,
                        0.6,
                        0.0,
                        0.0,
                        -1.0,
                    ] {
                        output.write_real(value);
                    }
                }
                output.close_data_chunk();
                output.open_data_chunk("ObjectsList", 3);
                output.close_data_chunk();
                let path = directory.path().join(name);
                std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
                path
            };
            let night_map = write_map("OwnedNight.map", 4, 0.1);
            let day_map = write_map("OwnedDay.map", 2, 0.5);
            let mut first = GameLogic::new();
            first.start_new_game(GameMode::Shell);
            assert!(first.load_map(night_map.to_str().unwrap()));
            let first_before = crate::presentation_frame::PresentationWorldEnv::from_logic(&first);
            assert!(first_before.is_night);
            let first_color = first_before.primary_object_lighting.unwrap().ambient;
            let mut second = GameLogic::new();
            second.start_new_game(GameMode::Shell);
            assert!(second.load_map(day_map.to_str().unwrap()));
            assert!(!crate::presentation_frame::PresentationWorldEnv::from_logic(&second).is_night);
            let first_after = crate::presentation_frame::PresentationWorldEnv::from_logic(&first);
            assert!(
                first_after.is_night,
                "another admitted map cannot change the first scene's TOD"
            );
            assert_eq!(
                first_after.primary_object_lighting.unwrap().ambient,
                first_color
            );
        },
    );
}

#[test]
fn actual_map_game_data_thresholds_reach_same_id_factory_bodies_without_cross_world_changes() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_map_game_data_thresholds_reach_same_id_factory_bodies_without_cross_world_changes",
        || {
            use crate::game_logic::host_enum_table_residual::HostBodyDamageType as State;
            use crate::game_logic::{GameLogic, GameMode, Player, Team};
            let directory = tempfile::tempdir().unwrap();
            let write_map = |name: &str, damaged: f32, really: f32| {
                let map_dir = directory.path().join(name);
                std::fs::create_dir(&map_dir).unwrap();
                let mut output = game_engine::common::system::DataChunkOutput::new();
                output.open_data_chunk("HeightMapData", 3);
                for value in [8, 8, 0, 64] {
                    output.write_int(value);
                }
                for _ in 0..64 {
                    output.write_byte(40);
                }
                output.close_data_chunk();
                output.open_data_chunk("ObjectsList", 3);
                output.close_data_chunk();
                let path = map_dir.join(format!("{name}.map"));
                std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
                std::fs::write(map_dir.join("Map.ini"),format!("GameData\nUnitDamagedThreshold = {damaged}\nUnitReallyDamagedThreshold = {really}\nEnd\n")).unwrap();
                path
            };
            let first_map = write_map("ThresholdFirst", 0.8, 0.4);
            let second_map = write_map("ThresholdSecond", 0.5, 0.2);
            let make_world = |path: &std::path::Path| {
                let mut world = GameLogic::new();
                world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
                    "map-threshold-body.ini".into(),
                    "Object MapThresholdBody\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 60\nEnd\nEnd\n".into()
                )]).unwrap()).unwrap();
                world.start_new_game(GameMode::Shell);
                assert!(world.load_map(path.to_str().unwrap()));
                world.add_player(Player::new(1, Team::USA, "Owner", true));
                let id = world
                    .create_object_for_player("MapThresholdBody", 1, glam::Vec3::ZERO)
                    .unwrap();
                (world, id)
            };
            let (mut first, id) = make_world(&first_map);
            assert_eq!(
                first.host_object(id).unwrap().body_damage_state,
                State::Damaged
            );
            let (mut second, peer) = make_world(&second_map);
            assert_eq!(id, peer);
            assert_eq!(
                second.host_object(peer).unwrap().body_damage_state,
                State::Pristine,
                "receiving Map.ini0.5/.2 must classify60/100 as pristine, not hardcoded retail"
            );
            first
                .host_objects_mut()
                .get_mut(&id)
                .unwrap()
                .internal_change_health(-25.0);
            second
                .host_objects_mut()
                .get_mut(&peer)
                .unwrap()
                .internal_change_health(-25.0);
            assert_eq!(
                first.host_object(id).unwrap().body_damage_state,
                State::ReallyDamaged
            );
            assert_eq!(
                second.host_object(peer).unwrap().body_damage_state,
                State::Damaged
            );
            drop(second);
            first
                .host_objects_mut()
                .get_mut(&id)
                .unwrap()
                .internal_change_health(25.0);
            assert_eq!(
                first.host_object(id).unwrap().body_damage_state,
                State::Damaged
            );
        },
    );
}
