//! Known child failures unwind real map/exact-script constructor callbacks.
use super::*;

fn world() -> GameLogic {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "checked-factory.ini".to_owned(),
        r#"
Object CheckedParent
 KindOf = STRUCTURE
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = GarrisonContain ContainTag
  ContainMax = 1
  AllowInsideKindOf = INFANTRY
  InitialRoster = CheckedBad 1
 End
End
Object CheckedBad
 KindOf = INFANTRY
 Body = ActiveBody BodyTag
  MaxHealth = NaN
  InitialHealth = 100
 End
End
Object CheckedAfter
 KindOf = VEHICLE
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
        .to_owned(),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    world
}
fn failure(error: &gamelogic::GameLogicError) {
    let error = error.to_string();
    assert!(
        error.contains("CheckedBad"),
        "actual rejected child identity: {error}"
    );
    assert!(
        error.contains("MaxHealth"),
        "actual Body contract, not a later native-capability miss: {error}"
    );
}
#[test]
fn checked_known_body_error_remains_distinct_from_unresolved_template() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "checked_known_body_error_remains_distinct_from_unresolved_template",
        || {
            let mut world = world();
            let error = world
                .create_map_object_with_effects_checked(
                    "CheckedBad",
                    Team::Neutral,
                    None,
                    Vec3::ZERO,
                    &mut Default::default(),
                )
                .unwrap_err();
            failure(&error);
            assert!(world.host_objects().is_empty());
            assert!(
                world
                    .create_map_object_with_effects_checked(
                        "DefinitelyAbsentCheckedTemplate",
                        Team::Neutral,
                        None,
                        Vec3::ZERO,
                        &mut Default::default()
                    )
                    .unwrap()
                    .is_none()
            );
        },
    );
}
#[test]
fn actual_map_rejects_nested_garrison_body_error_before_later_objects() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_map_rejects_nested_garrison_body_error_before_later_objects",
        || {
            let mut world = world();
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("CheckedNestedBody.map");
            let mut map = game_engine::common::system::DataChunkOutput::new();
            map.open_data_chunk("HeightMapData", 3);
            for value in [8, 8, 0, 64] {
                map.write_int(value);
            }
            for _ in 0..64 {
                map.write_byte(40);
            }
            map.close_data_chunk();
            map.open_data_chunk("ObjectsList", 3);
            for name in ["CheckedParent", "CheckedAfter"] {
                map.open_data_chunk("Object", 3);
                for value in [20.0, 20.0, 0.0, 0.0] {
                    map.write_real(value);
                }
                map.write_int(0);
                map.write_ascii_string(name);
                map.write_dict(&game_engine::common::dict::Dict::new());
                map.close_data_chunk();
            }
            map.close_data_chunk();
            std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
            assert!(!world.load_map_with_progress(path.to_str().unwrap(), |_, _| {}));
            assert!(!world.map_loaded);
            assert!(
                world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedParent"),
                "actual parent reached its constructor before child rejection"
            );
            assert!(
                !world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedBad")
            );
            assert!(
                !world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedAfter"),
                "later map objects must not run after constructor failure"
            );
        },
    );
}
#[test]
fn exact_script_propagates_nested_garrison_error_before_following_action() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "exact_script_propagates_nested_garrison_error_before_following_action",
        || {
            use gamelogic::scripting::core::{
                Parameter, ParameterType, ScriptAction, ScriptActionType,
            };
            let mut world = world();
            world.add_player(Player::new(0, Team::USA, "CheckedOwner", true));
            world
                .team_factory
                .lock()
                .unwrap()
                .init_team_owned(
                    "CheckedTeam".into(),
                    "CheckedOwner".into(),
                    false,
                    None,
                    Some(0),
                )
                .unwrap();
            let action = |name: &str| {
                let mut action = ScriptAction::new(ScriptActionType::CreateObject);
                for parameter in [
                    Parameter::with_string(ParameterType::ObjectType, name.into()),
                    Parameter::with_string(ParameterType::Team, "CheckedTeam".into()),
                    Parameter::with_coord(
                        ParameterType::Coord3D,
                        gamelogic::common::Coord3D::new(10., 20., 0.),
                    ),
                    Parameter::with_real(ParameterType::Angle, 0.),
                ] {
                    action.add_parameter(parameter).unwrap();
                }
                action
            };
            let mut first = action("CheckedParent");
            first.next_action = Some(Box::new(action("CheckedAfter")));
            let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
            let error = world
                .execute_checked_factory_actions_for_test(&engine, &first)
                .unwrap_err();
            failure(&error);
            assert!(
                world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedParent")
            );
            assert!(
                !world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedBad")
            );
            assert!(
                !world
                    .host_objects()
                    .values()
                    .any(|object| object.template_name == "CheckedAfter")
            );
            assert_eq!(engine.get_calling_team_id(), None);
        },
    );
}

#[cfg(feature = "game_client")]
#[test]
fn native_world_requires_receiver_but_same_id_headless_peer_remains_independent() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_world_requires_receiver_but_same_id_headless_peer_remains_independent",
        || {
            fn actor() -> ThingTemplate {
                let mut template = ThingTemplate::new("OwnerContractActor");
                template.add_kind_of(KindOf::Vehicle).set_health(100.0);
                template
            }
            let mut native = GameLogic::new();
            let mut headless = GameLogic::new();
            native
                .templates
                .insert("OwnerContractActor".into(), actor());
            headless
                .templates
                .insert("OwnerContractActor".into(), actor());
            native.require_native_drawable_owner();
            let requested = Vec3::new(37.0, 0.0, 53.0);
            let error = native
                .create_map_object_with_effects_checked(
                    "OwnerContractActor",
                    Team::Neutral,
                    None,
                    requested,
                    &mut Default::default(),
                )
                .unwrap_err();
            assert!(
                error.to_string().contains("native Drawable owner"),
                "{error}"
            );
            let failed = native.host_objects().values().next().unwrap();
            assert_eq!(
                failed.get_position(),
                Vec3::ZERO,
                "caller placement cannot run after rejected birth"
            );
            let id = headless
                .create_map_object_with_effects_checked(
                    "OwnerContractActor",
                    Team::Neutral,
                    None,
                    requested,
                    &mut Default::default(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(id, failed.id);
            assert_eq!(headless.host_object(id).unwrap().get_position(), requested);
            assert!(!headless.requires_native_drawable_owner());
        },
    );
}
