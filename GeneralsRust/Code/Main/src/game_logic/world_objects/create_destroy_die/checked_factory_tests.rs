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
            assert!(world.host_objects().registered_is_empty());
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
fn known_receiving_garrison_child_is_validated_without_a_host_template_cache_entry() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "known_receiving_garrison_child_is_validated_without_a_host_template_cache_entry",
        || {
            let mut world = world();
            world.add_player(
                Player::new(1, Team::USA, "CheckedOwner", true)
                    .with_script_identity("CheckedOwner"),
            );
            world.templates.remove("CheckedBad");
            assert!(!world.templates.contains_key("CheckedBad"));
            assert!(
                world
                    .receiving_module_content
                    .as_ref()
                    .unwrap()
                    .admitted_template("CheckedBad")
                    .is_some()
            );
            let parent = world.next_object_id_for_snapshot();
            let error = world
                .create_map_object_with_effects_checked(
                    "CheckedParent",
                    Team::USA,
                    Some(1),
                    Vec3::ZERO,
                    &mut Default::default(),
                )
                .unwrap_err();
            failure(&error);
            assert_eq!(
                world.next_object_id_for_snapshot(),
                ObjectId(parent.0.wrapping_add(1))
            );
            assert_eq!(
                world.host_object(parent).unwrap().template_name,
                "CheckedParent"
            );
            assert!(
                !world
                    .host_objects()
                    .registered_values()
                    .any(|actor| actor.template_name == "CheckedBad")
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
            #[cfg(feature = "game_client")]
            #[derive(Default)]
            struct ParentRetirement {
                retired: Vec<ObjectId>,
                parent_token: std::sync::Weak<()>,
                retirement_counts_at_discard: Vec<usize>,
            }
            #[cfg(feature = "game_client")]
            impl crate::game_logic::NativeViewEffects for ParentRetirement {
                #[cfg(feature = "game_client")]
                fn body_visual_changed(
                    &mut self,
                    change: &crate::game_logic::NativeBodyVisualChange,
                    audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
                ) -> Result<bool, String> {
                    // This recipient owns no native Drawable; no mixer admission occurs.
                    let _ = (change, audio);
                    Ok(false)
                }

                fn local_player_id(&self) -> u32 {
                    0
                }
                fn host_epoch(&self) -> u64 {
                    1
                }
                fn sample_slave(
                    &mut self,
                    _: &crate::game_logic::NativeSlaveSource,
                    _: &str,
                ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
                    panic!("a rejected map factory does not sample camera bones")
                }
                fn admit_factory_drawable(
                    &mut self,
                    source: crate::game_logic::NativeFactoryDrawableSource,
                    _: game_client::core::game_client::NativeDrawableBirthContext,
                    _: &mut game_engine::common::thing::module_factory::ModuleFactory,
                ) -> Result<(), String> {
                    panic!(
                        "Garrison InitialRoster rejection precedes native Drawable admission; actual template={}",
                        source.render.template_name
                    )
                }
                fn retire_physical_object_drawable(
                    &mut self,
                    source: &crate::game_logic::NativeObjectRetirement<'_>,
                ) -> Result<(), String> {
                    assert_eq!(source.object.template_name, "CheckedParent");
                    assert_eq!(source.object.health.current, 100.0);
                    assert!(source.object.status.destroyed);
                    assert!(
                        !source.object.status.on_die_started,
                        "failed map cleanup is direct deletion, not body death"
                    );
                    assert_ne!(source.object.visual_object_generation, 0);
                    assert!(!source.live_objects.contains_key(&source.object.id));
                    assert!(
                        !source
                            .live_objects
                            .ordered_ids()
                            .contains(&source.object.id)
                    );
                    self.parent_token =
                        std::sync::Arc::downgrade(&source.object.visual_draw_identity_token);
                    self.retired.push(source.object.id);
                    Ok(())
                }
            }
            #[cfg(feature = "game_client")]
            impl crate::game_logic::NativeMapAdmission for ParentRetirement {
                fn discard_attempt(&mut self) -> Result<(), String> {
                    self.retirement_counts_at_discard.push(self.retired.len());
                    Ok(())
                }
                fn on_level_start(&mut self) -> Result<(), String> {
                    panic!("rejected map must not publish level start")
                }
                fn prepare(
                    &mut self,
                    _: &GameLogic,
                    _: Option<&crate::game_logic::script_loader::MapMetadata>,
                ) -> Result<(), String> {
                    Ok(())
                }
                fn admit_restored_heights(&mut self, _: &GameLogic) -> Result<(), String> {
                    panic!("no saved height admission in map rejection fixture")
                }
                fn admit_restored_object_drawables(
                    &mut self,
                    _: &mut GameLogic,
                ) -> Result<(), String> {
                    panic!("no snapshot admission in map rejection fixture")
                }
                fn effects(&mut self) -> crate::game_logic::ScriptPresentationEffects<'_> {
                    crate::game_logic::ScriptPresentationEffects {
                        view: Some(self),
                        ..Default::default()
                    }
                }
            }
            #[cfg(feature = "game_client")]
            let mut receipt = ParentRetirement::default();
            #[cfg(feature = "game_client")]
            let native = Some(&mut receipt as &mut dyn crate::game_logic::NativeMapAdmission);
            #[cfg(not(feature = "game_client"))]
            let native = None;
            let parent_id = world.next_object_id_for_snapshot();
            assert!(
                !world
                    .load_map_with_progress_and_native_checked(
                        path.to_str().unwrap(),
                        |_, _| {},
                        native
                    )
                    .expect("nested Body rejection completes its map cleanup")
            );
            assert!(!world.map_loaded);
            assert_eq!(
                world.next_object_id_for_snapshot(),
                ObjectId(parent_id.0.wrapping_add(1)),
                "only the parent allocates: CheckedBad fails Body validation, following CheckedAfter never runs"
            );
            #[cfg(feature = "game_client")]
            {
                assert_eq!(
                    receipt.retired,
                    vec![parent_id],
                    "actual registered parent constructor is proved by its single physical retirement callback"
                );
                assert_eq!(
                    receipt.retirement_counts_at_discard,
                    vec![0, 1],
                    "logical parent retirement precedes rejected native attempt discard"
                );
                assert!(
                    receipt.parent_token.upgrade().is_none(),
                    "clean rejection leaves no parent lifetime owner"
                );
            }
            assert!(
                world.host_objects().registered_is_empty(),
                "rejected map retires its already constructed parent"
            );
            assert!(world.objects.ordered_ids().is_empty());
            assert!(!world.has_pending_object_removals());
            assert!(world.object_retirement_error().is_none());
            assert!(
                !world
                    .host_objects()
                    .registered_values()
                    .any(|object| object.template_name == "CheckedBad")
            );
            assert!(
                !world
                    .host_objects()
                    .registered_values()
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
                    .registered_values()
                    .any(|object| object.template_name == "CheckedParent")
            );
            assert!(
                !world
                    .host_objects()
                    .registered_values()
                    .any(|object| object.template_name == "CheckedBad")
            );
            assert!(
                !world
                    .host_objects()
                    .registered_values()
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
            let failed = native.host_objects().registered_values().next().unwrap();
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
