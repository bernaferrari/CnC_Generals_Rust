//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn named_flash_sets_drawable_flash_count_for_presentation() {
    use super::borrowed_presentation_owner::PresentationOwner;
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = PresentationOwner::new("FlashUnit", 8750);
    let mut peer = PresentationOwner::new("FlashUnit", 8750);
    let mut action = ScriptAction::new(ScriptActionType::NamedFlash);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FlashUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    let drawable = owner.drawable.as_mut().unwrap();
    assert_eq!(
        drawable.get_flash_count(),
        4,
        "2s * 30fps / 15 frames-per-flash"
    );
    assert_eq!(peer.drawable.as_ref().unwrap().get_flash_count(), 0);
    drawable.color_flash(Some(Color::white()), 9, 0, false);
    assert_eq!(
        drawable.get_flash_count(),
        4,
        "an unrelated tint pulse cannot overwrite the script countdown"
    );
    drawable.update(1.0 / 30.0, 14, 14).unwrap();
    assert_eq!(
        drawable.get_flash_count(),
        4,
        "configuration does not pulse early"
    );
    drawable.update(1.0 / 30.0, 15, 15).unwrap();
    assert_eq!(
        drawable.get_flash_count(),
        3,
        "original Drawable update admits one pulse"
    );
    drawable.update(1.0 / 30.0, 15, 15).unwrap();
    assert_eq!(
        drawable.get_flash_count(),
        3,
        "repeating a client frame does not emit another pulse"
    );
    // Persist the actual Drawable, then continue its admitted pulse schedule.
    let mut saved = Vec::new();
    crate::common::Snapshot::xfer(
        drawable,
        &mut game_engine::common::system::xfer_save::XferSave::new(
            std::io::Cursor::new(&mut saved),
            1,
        ),
    );
    let mut restored = crate::object::drawable::Drawable::new(
        1,
        8750,
        "FlashModel".into(),
        crate::object::drawable::DrawableType::Static,
    );
    crate::common::Snapshot::xfer(
        &mut restored,
        &mut game_engine::common::system::xfer_load::XferLoad::new(std::io::Cursor::new(saved), 1),
    );
    assert_eq!(restored.get_flash_count(), 3);
    drawable.update(1.0 / 30.0, 30, 30).unwrap();
    restored.update(1.0 / 30.0, 30, 30).unwrap();
    assert_eq!(drawable.get_flash_count(), 2);
    assert_eq!(restored.get_flash_count(), 2);
    peer.drawable = None;
    dispatcher
        .execute_action_with_driver(&action, &mut peer)
        .unwrap();
    assert!(
        peer.drawable.is_none(),
        "missing Drawable is an authoritative no-op"
    );
}

#[test]
fn named_custom_color_unpacks_argb_without_swapping_red_blue() {
    use super::borrowed_presentation_owner::PresentationOwner;
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = PresentationOwner::new("ColorUnit", 8760);
    let peer = PresentationOwner::new("ColorUnit", 8760);
    let old = peer.object.get_indicator_color();
    let mut action = ScriptAction::new(ScriptActionType::NamedCustomColor);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ColorUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Color, -65536))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    let color = owner.object.get_indicator_color();
    assert_eq!((color.r, color.g, color.b, color.a), (255, 0, 0, 255));
    assert_eq!(
        peer.object.get_indicator_color(),
        old,
        "same ID peer unchanged"
    );
}

#[test]
fn named_set_attitude_reads_int_mood() {
    #[derive(Debug, Default)]
    struct OwnedAi {
        attitudes: Vec<crate::modules::AIAttitudeType>,
    }
    impl AIUpdateInterface for OwnedAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            false
        }
        fn is_idle(&self) -> bool {
            true
        }
        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }
        fn set_attitude(
            &mut self,
            attitude: crate::modules::AIAttitudeType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.attitudes.push(attitude);
            Ok(())
        }
    }
    struct MoodOwner {
        object: crate::object::Object,
        ai: OwnedAi,
    }
    impl ScriptExecutionDriver for MoodOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_attitude(
            &mut self,
            unit: &str,
            _this: Option<ObjectID>,
            mood: i32,
        ) -> Option<GameLogicResult<()>> {
            if unit != "MoodUnit" {
                return Some(Ok(()));
            }
            let attitude = match mood {
                -2 => crate::modules::AIAttitudeType::Sleep,
                -1 => crate::modules::AIAttitudeType::Passive,
                1 => crate::modules::AIAttitudeType::Defensive,
                2 => crate::modules::AIAttitudeType::Aggressive,
                _ => crate::modules::AIAttitudeType::Normal,
            };
            let result = crate::scripting::executor::actions_named::issue_borrowed_named_attitude(
                &mut self.ai,
                attitude,
            )
            .map_err(|error| GameLogicError::Configuration(error.to_string()));
            Some(result)
        }
    }
    fn owner() -> MoodOwner {
        MoodOwner {
            object: crate::object::Object::new_raw(
                Arc::new(crate::common::DefaultThingTemplate::new("MoodUnit".into())),
                8770,
                crate::common::ObjectStatusMaskType::NONE,
                None,
            ),
            ai: OwnedAi::default(),
        }
    }
    let mut owner = owner();
    let peer = MoodOwner {
        object: crate::object::Object::new_raw(
            Arc::new(crate::common::DefaultThingTemplate::new("MoodUnit".into())),
            8770,
            crate::common::ObjectStatusMaskType::NONE,
            None,
        ),
        ai: OwnedAi::default(),
    };
    assert_eq!(owner.object.get_id(), peer.object.get_id());
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::NamedSetAttitude);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MoodUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::AiMood, 2))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(
        owner.ai.attitudes,
        vec![crate::modules::AIAttitudeType::Aggressive]
    );
    assert!(peer.ai.attitudes.is_empty(), "same-ID peer unchanged");
}

#[test]
fn named_attack_area_qualifies_my_inner_perimeter() {
    use crate::scripting::engine::ScriptExecutionDriver;
    struct AreaIngress {
        seen: Vec<(String, String, Option<String>)>,
        reject: bool,
    }
    impl ScriptExecutionDriver for AreaIngress {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_attack_area(
            &mut self,
            unit: &str,
            area: &str,
            player: Option<&str>,
            _: Option<ObjectID>,
        ) -> Option<GameLogicResult<()>> {
            self.seen
                .push((unit.into(), area.into(), player.map(str::to_owned)));
            Some(if self.reject {
                Err(GameLogicError::ModuleError("owner rejects area".into()))
            } else {
                Ok(())
            })
        }
    }
    // Qualification needs the driving player's start index. The dispatcher
    // forwards the token; the actual Main owner regression verifies resolution.
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(Some("PerimeterOwner".into()), None);
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::NamedAttackArea);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "PerimeterAttacker".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            crate::scripting::engine::MY_INNER_PERIMETER.into(),
        ))
        .unwrap();
    let mut owner = AreaIngress {
        seen: Vec::new(),
        reject: false,
    };
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(
        owner.seen,
        vec![(
            "PerimeterAttacker".into(),
            crate::scripting::engine::MY_INNER_PERIMETER.into(),
            Some("PerimeterOwner".into())
        )]
    );
    owner.reject = true;
    assert!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .is_err(),
        "explicit rejection never falls through to ambient terrain"
    );
}

#[test]
fn team_follow_waypoints_honors_as_team_int() {
    use crate::scripting::engine::ScriptExecutionDriver;
    struct PathIngress {
        seen: Vec<(bool, bool)>,
        reject: bool,
    }
    impl ScriptExecutionDriver for PathIngress {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_waypoint_path(
            &mut self,
            team: &str,
            path: &str,
            as_team: bool,
            exact: bool,
            _: Option<crate::team::TeamID>,
            _: Option<crate::team::TeamID>,
        ) -> Option<GameLogicResult<()>> {
            assert_eq!((team, path), ("FollowTeam", "TeamPath"));
            self.seen.push((as_team, exact));
            Some(if self.reject {
                Err(GameLogicError::ModuleError("owner rejects path".into()))
            } else {
                Ok(())
            })
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = PathIngress {
        seen: Vec::new(),
        reject: false,
    };
    for (kind, flag) in [
        (ScriptActionType::TeamFollowWaypoints, 0),
        (ScriptActionType::TeamFollowWaypoints, 1),
        (ScriptActionType::TeamFollowWaypointsExact, -2),
    ] {
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "FollowTeam".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                "TeamPath".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, flag))
            .unwrap();
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap();
        owner.reject = true;
        assert!(
            dispatcher
                .execute_action_with_driver(&action, &mut owner)
                .is_err()
        );
        owner.reject = false;
    }
    assert_eq!(
        owner.seen,
        vec![
            (false, false),
            (false, false),
            (true, false),
            (true, false),
            (true, true),
            (true, true)
        ]
    );
}

#[test]
fn named_face_named_selects_borrowed_driver() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};

    struct NamedOwner {
        calls: Vec<(String, String, Option<ObjectID>)>,
        reject: bool,
    }
    impl ScriptExecutionDriver for NamedOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }

        fn named_command(
            &mut self,
            request: ScriptNamedCommand<'_>,
            this_object: Option<ObjectID>,
        ) -> Option<GameLogicResult<()>> {
            let ScriptNamedCommand::FaceObject { unit, target } = request else {
                panic!("unexpected named command: {request:?}");
            };
            self.calls.push((unit.into(), target.into(), this_object));
            Some(if self.reject {
                Err(GameLogicError::ModuleError(
                    "named owner rejected command".into(),
                ))
            } else {
                Ok(())
            })
        }
    }

    let engine = ScriptEngine::new().expect("script engine");
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedOwner {
        calls: Vec::new(),
        reject: false,
    };
    let mut action = ScriptAction::new(ScriptActionType::NamedFaceNamed);
    for name in ["FaceUnit", "FaceTarget"] {
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
            .unwrap();
    }
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![("FaceUnit".into(), "FaceTarget".into(), None)]
    );

    // A selected owner's failure is authoritative; the standalone adapter
    // must not turn it into Success or enqueue the command for another world.
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(message)) if message.contains("named owner rejected command")
    ));
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn video_completion_requires_notification_and_is_consumed() {
    let engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut condition = Condition::new(ConditionType::HasFinishedVideo);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Movie,
            "IntroMovie".into(),
        ))
        .unwrap();
    // ScriptEngine.cpp:7241-7264 consumes a notified video, not an audio timer.
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False
    );
    engine.notify_of_completed_video("IntroMovie");
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn speech_and_audio_completion_use_owned_deadlines_without_handler() {
    let engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");
    // ScriptEngine.cpp:7268-7330: retained timers belong to this engine.
    // Seed the serialized continuation, avoiding global audio catalog access.
    let mut tail = engine.snapshot_xfer_tail();
    tail.testing_speech = vec![("Briefing".into(), 13)];
    tail.testing_audio = vec![("Explosion".into(), 13)];
    engine.restore_xfer_tail(&tail);
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    for (kind, parameter, name) in [
        (
            ConditionType::HasFinishedSpeech,
            ParameterType::Dialog,
            "Briefing",
        ),
        (
            ConditionType::HasFinishedAudio,
            ParameterType::Sound,
            "Explosion",
        ),
    ] {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(parameter, name.into()))
            .unwrap();
        for frame in [10, 12] {
            state.borrow_mut().current_frame = frame;
            assert_eq!(
                evaluator.evaluate_condition(&mut condition).unwrap(),
                ScriptConditionResult::False
            );
            let retained = engine.snapshot_xfer_tail();
            let timers = if kind == ConditionType::HasFinishedSpeech {
                retained.testing_speech
            } else {
                retained.testing_audio
            };
            assert_eq!(timers, [(name.into(), 13)]);
        }
        state.borrow_mut().current_frame = 13;
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::True
        );
        let after = engine.snapshot_xfer_tail();
        let timers = if kind == ConditionType::HasFinishedSpeech {
            after.testing_speech
        } else {
            after.testing_audio
        };
        assert!(timers.is_empty(), "completed timer is consumed");
    }
}

#[test]
fn multiplayer_player_defeat_has_no_player_param() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptConditions.cpp:1748-1750 — no params; missing local player is false, not error.
    let mut condition = Condition::new(ConditionType::MultiplayerPlayerDefeat);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False
    );
}

// Driver contract fixture; Main named_lifecycle_owner_tests exercise real
// creation/removal/rename and snapshot history on the authoritative owner.
struct NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle);
impl crate::scripting::engine::ScriptExecutionDriver for NamedLifetimeOwner {
    fn after_action(&mut self) -> crate::GameLogicResult<()> {
        Ok(())
    }
    fn named_lifecycle(
        &self,
        _: &str,
        _: Option<u32>,
    ) -> crate::scripting::engine::ScriptOwnerQuery<crate::scripting::engine::ScriptNamedLifecycle>
    {
        crate::scripting::engine::ScriptOwnerQuery::Present(self.0)
    }
}

#[test]
fn named_totally_dead_false_while_object_exists() {
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle {
        exists: true,
        effectively_dead: true,
        ever_existed: true,
    });
    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "DeadHero".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "NAMED_TOTALLY_DEAD is false while the object still exists"
    );
    owner.0.exists = false;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "NAMED_TOTALLY_DEAD is true only after the named object is gone"
    );
}

#[test]
fn named_totally_dead_false_if_name_never_existed() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle {
        exists: false,
        effectively_dead: false,
        ever_existed: false,
    });
    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NeverExisted".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    owner.0.ever_existed = true;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
}

#[test]
fn unit_health_uses_initial_health_rounding() {
    // Driver ingress contract; Main condition_body_census_tests checks the
    // actual InitialHealth rounding against live owned bodies and peer worlds.
    struct BodyIngress(crate::scripting::engine::ScriptOwnerQuery<i32>);
    impl crate::scripting::engine::ScriptExecutionDriver for BodyIngress {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn unit_health_percent(
            &self,
            name: &str,
            _: Option<ObjectID>,
        ) -> crate::scripting::engine::ScriptOwnerQuery<i32> {
            assert_eq!(name, "HurtHero");
            self.0
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut condition = Condition::new(ConditionType::UnitHealth);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "HurtHero".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    let mut owner = BodyIngress(crate::scripting::engine::ScriptOwnerQuery::Present(100));
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
    owner.0 = crate::scripting::engine::ScriptOwnerQuery::Present(99);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    owner.0 = crate::scripting::engine::ScriptOwnerQuery::Missing;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn built_by_player_rejects_object_type_lists() {
    // C++ ScriptConditions.cpp:872-874 findTemplate(raw) must exist.
    struct Owner;
    impl crate::scripting::engine::ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn literal_template_exists(
            &self,
            name: &str,
        ) -> crate::scripting::engine::ScriptOwnerQuery<bool> {
            crate::scripting::engine::ScriptOwnerQuery::Present(name == "AmericaRanger")
        }
        fn player_census(
            &self,
            _: &str,
            _: Option<&str>,
        ) -> crate::scripting::engine::ScriptOwnerQuery<(
            i32,
            crate::scripting::HostScriptPlayerCensus,
        )> {
            let mut census = crate::scripting::HostScriptPlayerCensus::default();
            census.template_counts.insert("americaranger".into(), 1);
            crate::scripting::engine::ScriptOwnerQuery::Present((0, census))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let mut list = crate::object::object_types::ObjectTypes::new();
    list.add_object_type(crate::common::AsciiString::from("AmericaRanger"));
    engine.set_object_types("HeroList".into(), list);
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    for (name, expected) in [
        ("HeroList", ScriptConditionResult::False),
        ("AmericaRanger", ScriptConditionResult::True),
    ] {
        let mut condition = Condition::new(ConditionType::BuiltByPlayer);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                name.into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "PlyrAmerica".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut Owner)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn team_the_player_not_remapped_outside_challenge() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptEngine.cpp:5935-5939 remaps TEAM_THE_PLAYER only in Challenge.
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator.resolve_string_token(crate::scripting::core::TEAM_THE_PLAYER),
        crate::scripting::core::TEAM_THE_PLAYER
    );
}

#[test]
fn the_player_not_remapped_outside_challenge() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptEngine.cpp:5809-5814 remaps THE_PLAYER only in Challenge.
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator.resolve_string_token(crate::scripting::core::THE_PLAYER),
        crate::scripting::core::THE_PLAYER
    );
}

#[test]
fn player_has_counts_dead_lost_ignores_dead() {
    // C++ HAS ignoreDead=FALSE (1792), LOST ignoreDead=TRUE (2686).
    let mut player = crate::player::Player::new(0);
    const ID: u32 = 0x0A5_DEAD;
    let template: Arc<dyn crate::common::ThingTemplate> = Arc::new(
        crate::common::DefaultThingTemplate::new("AmericaRanger".into()),
    );
    let mut dead = crate::object::Object::new_test_from_template(ID, 100.0, Arc::clone(&template));
    dead.set_effectively_dead(true);
    let mut peer = crate::object::Object::new_test_from_template(ID, 100.0, Arc::clone(&template));
    player.add_owned_object_for_object(&dead);
    let templates = vec![Arc::clone(&template), Arc::clone(&template)];
    let mut counts = [99, 99];
    player.count_objects_by_thing_template_owned(&templates, false, true, &mut counts, |id| {
        (id == ID).then_some(&dead)
    });
    assert_eq!(
        counts,
        [1, 0],
        "HAS counts dead bodies once, at first equivalent template"
    );
    player.count_objects_by_thing_template_owned(&templates, true, true, &mut counts, |id| {
        (id == ID).then_some(&dead)
    });
    assert_eq!(counts, [0, 0], "LOST excludes effectively-dead bodies");
    player.count_objects_by_thing_template_owned(&templates, true, true, &mut counts, |id| {
        (id == ID).then_some(&peer)
    });
    assert_eq!(
        counts,
        [1, 0],
        "same ID in another supplied store has its own body state"
    );
    peer.set_status(
        crate::prelude::ObjectStatusTypes::UnderConstruction.into(),
        true,
    );
    player.count_objects_by_thing_template_owned(&templates, false, true, &mut counts, |id| {
        (id == ID).then_some(&peer)
    });
    assert_eq!(counts, [0, 0]);
    player.count_objects_by_thing_template_owned(&templates, false, false, &mut counts, |id| {
        (id == ID).then_some(&peer)
    });
    assert_eq!(
        counts,
        [1, 0],
        "under-construction inclusion follows explicit policy"
    );
    player.count_objects_by_thing_template_owned(&templates, false, false, &mut counts, |_| None);
    assert_eq!(
        counts,
        [0, 0],
        "missing owner entries cannot consult an ambient registry"
    );
}

#[test]
fn skirmish_value_in_area_excludes_inert() {
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = OwnedAreaCensus::new(vec![owned_area_unit(
        17,
        "PlyrAmerica",
        "InertProp",
        5.0,
        5.0,
        &["Inert"],
    )]);
    let mut condition = Condition::new(ConditionType::SkirmishValueInArea);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 4))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "inert objects do not add value"
    );
}

#[test]
fn set_cave_index_uses_supplied_owner() {
    use crate::scripting::engine::ScriptExecutionDriver;
    struct Owner(Option<(String, i32)>);
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn set_named_cave_index(&mut self, name: &str, value: i32) -> Option<()> {
            self.0 = Some((name.into(), value));
            Some(())
        }
    }
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(42));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::SetCaveIndex);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, "CaveB".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    let mut owner = Owner(None);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.0, Some(("CaveB".into(), 3)));
}

#[test]
fn team_panic_queues_host_when_dual_world_empty() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    struct Owner {
        route: Option<(String, String)>,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_panic(
            &mut self,
            team: &str,
            path: &str,
            _: Option<u32>,
            _: Option<u32>,
        ) -> Option<GameLogicResult<()>> {
            self.route = Some((team.into(), path.into()));
            Some(Ok(()))
        }
    }
    let mut owner = Owner { route: None };
    let mut action = ScriptAction::new(ScriptActionType::TeamPanic);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TeamCivilians".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "PanicPath".into(),
        ))
        .unwrap();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.route,
        Some(("TeamCivilians".into(), "PanicPath".into()))
    );
}

#[test]
fn create_object_admits_only_the_borrowed_creation_owner() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptObjectCreation};
    struct Owner {
        factory: crate::team::TeamFactory,
        object: Option<crate::object::Object>,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn create_object(
            &mut self,
            request: ScriptObjectCreation<'_>,
            _: Option<u32>,
            _: Option<u32>,
        ) -> Option<GameLogicResult<()>> {
            let Some((team, effects)) = self
                .factory
                .create_inactive_team_with_owned_effects(request.team)
            else {
                return Some(Ok(()));
            };
            assert!(effects.is_empty());
            let mut object = crate::object::Object::new_raw(
                Arc::new(crate::common::DefaultThingTemplate::new(
                    request.template.into(),
                )),
                8790,
                crate::common::ObjectStatusMaskType::NONE,
                Some(team.clone()),
            );
            object.set_orientation(request.angle).unwrap();
            object
                .set_position(&crate::common::Coord3D::new(
                    request.x, request.y, request.z,
                ))
                .unwrap();
            team.write().unwrap().add_member(object.get_id());
            self.object = Some(object);
            Some(Ok(()))
        }
    }
    let mut owner = Owner {
        factory: crate::team::TeamFactory::new(),
        object: None,
    };
    let mut peer = Owner {
        factory: crate::team::TeamFactory::new(),
        object: None,
    };
    for source in [&mut owner, &mut peer] {
        source
            .factory
            .init_team_owned("teamAmerica".into(), "Owner".into(), false, None, Some(7))
            .unwrap();
    }
    let dispatch_engine = ScriptEngine::new().unwrap();
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaInfantryRanger".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            crate::scripting::core::Coord3D::new(10.0, 20.0, 0.0),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Angle, 1.5))
        .unwrap();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    let object = owner.object.as_ref().unwrap();
    assert_eq!(object.get_position().x, 10.0);
    assert_eq!(object.get_position().y, 20.0);
    assert_eq!(object.get_position().z, 0.0);
    assert_eq!(object.get_orientation(), 1.5);
    assert_eq!(
        object
            .get_team()
            .unwrap()
            .read()
            .unwrap()
            .get_controlling_player_id(),
        Some(7)
    );
    assert!(
        owner.factory.find_team_instances("teamAmerica")[0]
            .read()
            .unwrap()
            .has_member(8790)
    );
    assert!(peer.object.is_none());
    assert!(peer.factory.find_team_instances("teamAmerica").is_empty());
}

#[test]
fn named_team_lifecycle_uses_supplied_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut owner = LifecycleOwner::default();

    let mut named_delete = ScriptAction::new(ScriptActionType::NamedDelete);
    named_delete
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Flyover".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_delete, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut named_kill = ScriptAction::new(ScriptActionType::NamedKill);
    named_kill
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Civilian".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_kill, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut named_damage = ScriptAction::new(ScriptActionType::NamedDamage);
    named_damage
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    named_damage
        .add_parameter(Parameter::with_int(ParameterType::Int, 25))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_damage, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_delete = ScriptAction::new(ScriptActionType::TeamDelete);
    team_delete
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_delete, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_kill = ScriptAction::new(ScriptActionType::TeamKill);
    team_kill
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamChina".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_kill, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_damage = ScriptAction::new(ScriptActionType::DamageMembersOfTeam);
    team_damage
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamGLA".into(),
        ))
        .unwrap();
    team_damage
        .add_parameter(Parameter::with_real(ParameterType::Real, 40.0))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_damage, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_delete_living = ScriptAction::new(ScriptActionType::TeamDeleteLiving);
    team_delete_living
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamUSA".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_delete_living, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.effects,
        vec![
            HostScriptKillDeleteDamageRequest::NamedDelete {
                unit: "Flyover".to_string()
            },
            HostScriptKillDeleteDamageRequest::NamedKill {
                unit: "Civilian".to_string()
            },
            HostScriptKillDeleteDamageRequest::NamedDamage {
                unit: "Hero".to_string(),
                amount: 25
            },
            HostScriptKillDeleteDamageRequest::TeamDelete {
                team: "teamAmerica".to_string(),
                ignore_dead: false
            },
            HostScriptKillDeleteDamageRequest::TeamKill {
                team: "teamChina".to_string()
            },
            HostScriptKillDeleteDamageRequest::TeamDamage {
                team: "teamGLA".to_string(),
                amount: 40.0
            },
            HostScriptKillDeleteDamageRequest::TeamDelete {
                team: "teamUSA".to_string(),
                ignore_dead: true
            },
        ]
    );
}

#[test]
fn player_kill_uses_supplied_owner_even_when_missing() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut owner = LifecycleOwner::default();
    let mut action = ScriptAction::new(ScriptActionType::PlayerKill);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.players,
        vec![HostScriptPlayerMiscRequest::Kill {
            player: "PlyrGLA".to_string()
        }]
    );
}

#[test]
fn named_object_sound_uses_supplied_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut owner = ObjectEffectOwner::default();

    let mut play = ScriptAction::new(ScriptActionType::SoundPlayNamed);
    play.add_parameter(Parameter::with_string(
        ParameterType::TextString,
        "UnitCheer".into(),
    ))
    .unwrap();
    play.add_parameter(Parameter::with_string(
        ParameterType::Unit,
        "NamedScout".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&play, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut enable = ScriptAction::new(ScriptActionType::EnableObjectSound);
    enable
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedFactory".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&enable, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut disable = ScriptAction::new(ScriptActionType::DisableObjectSound);
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedFactory".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&disable, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.sound,
        vec![
            HostScriptObjectSoundRequest::PlayNamed {
                sound: "UnitCheer".into(),
                unit: "NamedScout".into()
            },
            HostScriptObjectSoundRequest::Enable {
                unit: "NamedFactory".to_string(),
                enable: true
            },
            HostScriptObjectSoundRequest::Enable {
                unit: "NamedFactory".to_string(),
                enable: false
            },
        ]
    );
}

#[test]
fn team_set_attitude_uses_borrowed_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut action = ScriptAction::new(ScriptActionType::TeamSetAttitude);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "AmericaTeamHeroes".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::AiMood, 2))
        .unwrap();
    let mut owner = super::borrowed_order_owner::OrderOwner::default();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.attitudes, vec![("AmericaTeamHeroes".to_string(), 2)]);
}

#[test]
fn build_team_queues_host_when_dual_world_empty() {
    use crate::scripting::engine::{ScriptAiPlayerRequest, ScriptExecutionDriver};
    // Typed ingress contract; Main script_ai_build_owner_tests covers actual
    // AI queues, authored layouts and recruitment on the driving player.
    struct BuildIngress {
        calls: usize,
        accepted: usize,
        reject: bool,
    }
    impl ScriptExecutionDriver for BuildIngress {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn ai_player(&mut self, request: ScriptAiPlayerRequest<'_>) -> Option<GameLogicResult<()>> {
            assert_eq!(
                request,
                ScriptAiPlayerRequest::BuildTeam { team: "SquadHost" }
            );
            self.calls += 1;
            Some(if self.reject {
                Err(GameLogicError::ModuleError(
                    "owned AI rejects prototype".into(),
                ))
            } else {
                self.accepted += 1;
                Ok(())
            })
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::BuildTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "SquadHost".into(),
        ))
        .unwrap();
    let mut owner = BuildIngress {
        calls: 0,
        accepted: 0,
        reject: false,
    };
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(owner.calls, 1);
    assert_eq!(owner.accepted, 1);
    owner.reject = true;
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success,
        "CPP's void build action retains success after its owned AI rejects the request"
    );
    assert_eq!(
        owner.calls, 2,
        "rejection is handled by the same explicit AI owner"
    );
    assert_eq!(
        owner.accepted, 1,
        "rejected builds do not enter another roster"
    );
}

#[test]
fn ai_player_build_actions_queue_host_when_dual_world_empty() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut owner = OwnedAiActions::default();

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);

    let mut supply = ScriptAction::new(ScriptActionType::AiPlayerBuildSupplyCenter);
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaSupplyCenter".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_int(ParameterType::Int, 1000))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&supply, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut upgrade = ScriptAction::new(ScriptActionType::AiPlayerBuildUpgrade);
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_AmericaSupplyLines".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&upgrade, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut nearest = ScriptAction::new(ScriptActionType::AiPlayerBuildTypeNearestTeam);
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaPatriotBattery".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&nearest, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.effects,
        vec![
            (
                "supply".into(),
                vec![
                    "PlyrAmerica".into(),
                    "AmericaSupplyCenter".into(),
                    "1000".into()
                ]
            ),
            (
                "upgrade".into(),
                vec!["PlyrAmerica".into(), "Upgrade_AmericaSupplyLines".into()]
            ),
            (
                "nearest".into(),
                vec![
                    "PlyrAmerica".into(),
                    "AmericaPatriotBattery".into(),
                    "USA_RangerSquad".into()
                ]
            ),
        ]
    );
}

#[test]
fn live_named_created_true_from_host_snapshot() {
    // Owner-ingress contract; Main named_lifecycle_owner_tests verifies actual
    // creation, renaming, destruction, save/load and same-ID peer isolation.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle {
        exists: true,
        effectively_dead: false,
        ever_existed: true,
    });
    let mut condition = Condition::new(ConditionType::NamedCreated);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
    owner.0.exists = false;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn enemy_and_type_sighted_dispatch_to_the_owned_query() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptSightingFilter};
    struct SightingOwner {
        visible: bool,
    }
    impl ScriptExecutionDriver for SightingOwner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn sighted(
            &self,
            unit: &str,
            player: &str,
            filter: ScriptSightingFilter<'_>,
            current_player: Option<&str>,
            this_object: Option<u32>,
        ) -> ScriptOwnerQuery<bool> {
            assert_eq!(unit, "LookerScout");
            assert_eq!(player, "PlyrGLA");
            assert_eq!(current_player, None);
            assert_eq!(this_object, None);
            match filter {
                ScriptSightingFilter::Relationship(alliance) => assert_eq!(alliance, 0),
                ScriptSightingFilter::Types(names) => assert_eq!(names, &["AmericaRanger"]),
            }
            ScriptOwnerQuery::Present(self.visible)
        }
    }
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    let mut owner = SightingOwner { visible: true };
    let mut enemy_sighted = Condition::new(ConditionType::EnemySighted);
    enemy_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "LookerScout".into(),
        ))
        .unwrap();
    enemy_sighted
        .add_parameter(Parameter::with_int(ParameterType::Relation, 0))
        .unwrap();
    enemy_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut enemy_sighted, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "selected EnemySighted owner is authoritative"
    );

    let mut type_sighted = Condition::new(ConditionType::TypeSighted);
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "LookerScout".into(),
        ))
        .unwrap();
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaRanger".into(),
        ))
        .unwrap();
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut type_sighted, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "selected TypeSighted owner is authoritative"
    );

    // Contract test for live owner result changes. Main sighting_owner_tests
    // exercise actual geometry, relationship, visibility and lifecycle rules.
    owner.visible = false;
    for condition in [&mut enemy_sighted, &mut type_sighted] {
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
    }
}

#[test]
fn live_named_totally_dead_false_at_load_when_host_unit_lives() {
    // Main named_lifecycle_owner_tests covers the real persisted continuation.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle {
        exists: true,
        effectively_dead: false,
        ever_existed: true,
    });
    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapHero".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    owner.0.exists = false;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
}

#[test]
fn live_named_destroyed_true_after_host_destroy_list() {
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut owner = NamedLifetimeOwner(crate::scripting::engine::ScriptNamedLifecycle {
        exists: false,
        effectively_dead: false,
        ever_existed: true,
    });
    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapHero".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "known removed name is destroyed while unrelated objects may remain"
    );
    owner.0.ever_existed = false;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "never-created name must not be treated as destroyed"
    );
}

#[test]
fn named_selected_uses_actual_owned_selection_changes_and_cpp_cache() {
    use super::borrowed_selection_owner::SelectionOwner;
    let engine = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut owner = SelectionOwner::new("TutorialRanger", 11);
    let mut peer = SelectionOwner::new("TutorialRanger", 11);
    let mut condition = Condition::new(ConditionType::NamedSelected);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TutorialRanger".into(),
        ))
        .unwrap();
    owner.select(7);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!((condition.custom_data, condition.custom_frame), (1, 7));
    peer.select(8);
    peer.clear(9);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
    owner.clear(7);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "CPP intentionally keeps the cached result for same-frame changes"
    );
    owner.select(8);
    owner.clear(9);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    assert_eq!((condition.custom_data, condition.custom_frame), (-1, 9));
    owner.multiplayer = true;
    owner.select(10);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    assert_eq!(
        (condition.custom_data, condition.custom_frame),
        (-1, 9),
        "multiplayer does not mutate cached state"
    );
}

#[test]
fn live_team_destroyed_false_when_host_members_live() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptTeamPresence};
    struct PresenceOwner(bool);
    impl ScriptExecutionDriver for PresenceOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_presence(
            &self,
            name: &str,
            _: Option<crate::team::TeamID>,
            _: Option<crate::team::TeamID>,
        ) -> ScriptOwnerQuery<ScriptTeamPresence> {
            if name != "USA_RangerSquad" {
                return ScriptOwnerQuery::Missing;
            }
            ScriptOwnerQuery::Present(ScriptTeamPresence {
                created: self.0,
                has_objects: self.0,
            })
        }
        fn team_has_units(
            &self,
            name: &str,
            _: Option<crate::team::TeamID>,
            _: Option<crate::team::TeamID>,
        ) -> ScriptOwnerQuery<bool> {
            if name != "USA_RangerSquad" {
                return ScriptOwnerQuery::Missing;
            }
            ScriptOwnerQuery::Present(self.0)
        }
    }
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut owner = PresenceOwner(true);
    for (kind, live, empty) in [
        (ConditionType::TeamDestroyed, false, true),
        (ConditionType::TeamHasUnits, true, false),
        (ConditionType::TeamCreated, true, false),
    ] {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "USA_RangerSquad".into(),
            ))
            .unwrap();
        owner.0 = true;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            if live {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            }
        );
        owner.0 = false;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            if empty {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            }
        );
        let mut missing = Condition::new(kind);
        missing
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "MissingTeam".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut missing, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
    }
    // Main team_presence_owner_tests covers the actual factory, canonical roster,
    // created phase, kind filters and contextual/multiple-instance selection.
}

#[test]
fn live_named_entered_uses_host_trigger_flags() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
    struct EntryOwner {
        world: crate::scripting::HostTriggerWorld,
        frame: u32,
    }
    impl ScriptExecutionDriver for EntryOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_entered_area(
            &self,
            unit: &str,
            area: &str,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<bool> {
            if unit != "Scout" {
                return ScriptOwnerQuery::Missing;
            }
            let Some(trigger) = self.world.trigger_area_by_canonical_name(area) else {
                return ScriptOwnerQuery::Missing;
            };
            ScriptOwnerQuery::Present(crate::scripting::host_object_did_enter(
                &self.world,
                7,
                &trigger,
                self.frame,
            ))
        }
    }
    let trigger = crate::polygon_trigger::PolygonTrigger::new(
        1814,
        "Wave18PolyPad".into(),
        vec![
            crate::common::ICoord3D::new(0, 0, 0),
            crate::common::ICoord3D::new(20, 0, 0),
            crate::common::ICoord3D::new(0, 20, 0),
        ],
    );
    let mut owner = EntryOwner {
        world: Default::default(),
        frame: 20,
    };
    owner.world.set_trigger_areas(&[trigger]);
    owner.world.set_current_frame(20);
    owner
        .world
        .update_object_flags(7, 18.0, 18.0, 19, false, Some("teamUSA"));
    owner
        .world
        .update_object_flags(7, 2.0, 2.0, 20, false, Some("teamUSA"));
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(20));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut condition = Condition::new(ConditionType::NamedEnteredArea);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave18PolyPad".into(),
        ))
        .unwrap();
    for (frame, expected) in [
        (20, ScriptConditionResult::True),
        (21, ScriptConditionResult::True),
        (22, ScriptConditionResult::False),
    ] {
        owner.frame = frame;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn live_named_body_state_reads_host_snapshot() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    use crate::scripting::engine::{
        ScriptContainmentStatus, ScriptExecutionDriver, ScriptNamedLifecycle, ScriptOwnerQuery,
    };
    struct BodyOwner;
    impl ScriptExecutionDriver for BodyOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn unit_health_percent(&self, name: &str, _: Option<ObjectID>) -> ScriptOwnerQuery<i32> {
            if name == "Hero" {
                ScriptOwnerQuery::Present(100)
            } else {
                ScriptOwnerQuery::Missing
            }
        }
        fn named_owned_by_player(
            &self,
            unit: &str,
            player: &str,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<bool> {
            if unit == "Hero" {
                ScriptOwnerQuery::Present(player == "PlyrAmerica")
            } else {
                ScriptOwnerQuery::Missing
            }
        }
        fn named_containment(
            &self,
            unit: &str,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ScriptContainmentStatus> {
            if unit == "Hero" {
                ScriptOwnerQuery::Present(ScriptContainmentStatus { count: 0, max: 2 })
            } else {
                ScriptOwnerQuery::Missing
            }
        }
        fn named_lifecycle(
            &self,
            unit: &str,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ScriptNamedLifecycle> {
            ScriptOwnerQuery::Present(ScriptNamedLifecycle {
                exists: unit == "Hero",
                effectively_dead: false,
                ever_existed: unit == "Hero",
            })
        }
    }
    let mut owner = BodyOwner;
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

    let mut health = Condition::new(ConditionType::UnitHealth);
    health
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    health
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    health
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut health, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );

    let mut owned = Condition::new(ConditionType::NamedOwnedByPlayer);
    owned
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    owned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut owned, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );

    let mut empty = Condition::new(ConditionType::NamedBuildingIsEmpty);
    empty
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut empty, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );

    let mut slots = Condition::new(ConditionType::NamedHasFreeContainerSlots);
    slots
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut slots, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );

    let mut dying = Condition::new(ConditionType::NamedDying);
    dying
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut dying, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );

    // Actual Main condition_body_census, named ownership and containment
    // fixtures cover the real factory/roster and same-ID peer controls.
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_zero_matching() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // Given: live host census with no matching type in the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA compares the count
    // Then: count is 0 so ==0 is true and >=1 is false
    let mut owner = OwnedAreaCensus::new(vec![owned_area_unit(
        1,
        "PlyrAmerica",
        "AmericaTankCrusader",
        5.0,
        5.0,
        &["Vehicle"],
    )]);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    let mut eq_zero = eval_player_unit_type_in_area(2, 0, "AmericaInfantryRanger", "HoldZone");
    let mut ge_one = eval_player_unit_type_in_area(3, 1, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut eq_zero, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "0 matching units in the trigger must compare as count == 0"
    );
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut ge_one, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "0 matching units must not satisfy >= 1"
    );
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_n_units_ge_n() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // Given: N live host units of the scripted type inside the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA asks >= N
    // Then: the condition is true (C++ evaluatePlayerHasUnitTypeInArea)
    let mut owner = OwnedAreaCensus::new(vec![
        owned_area_unit(
            1,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            4.0,
            4.0,
            &["Infantry"],
        ),
        owned_area_unit(
            2,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            8.0,
            8.0,
            &["Infantry"],
        ),
        owned_area_unit(
            3,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            80.0,
            80.0,
            &["Infantry"],
        ),
        owned_area_unit(
            4,
            "PlyrChina",
            "AmericaInfantryRanger",
            6.0,
            6.0,
            &["Infantry"],
        ),
    ]);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    let mut ge_two = eval_player_unit_type_in_area(3, 2, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut ge_two, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "two matching live units in the trigger must satisfy >= 2"
    );
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_map_load_eq_zero_false() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // Given: map-load census with living units already standing in the zone
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA == 0
    // Then: false — leftover owned_objects must not report an empty world
    let mut owner = OwnedAreaCensus::new(vec![owned_area_unit(
        1,
        "PlyrAmerica",
        "AmericaInfantryRanger",
        5.0,
        5.0,
        &["Infantry"],
    )]);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    let mut eq_zero = eval_player_unit_type_in_area(2, 0, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut eq_zero, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "== 0 must stay false at map load while living units occupy the trigger"
    );
}

#[test]
fn eval_player_has_comparison_unit_kind_in_trigger_area_n_units_ge_n() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // Given: N live host infantry inside the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA asks >= N
    // Then: true (C++ evaluatePlayerHasUnitKindInArea)
    const KINDOF_INFANTRY: i32 = 4;
    let mut owner = OwnedAreaCensus::new(vec![
        owned_area_unit(
            1,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            4.0,
            4.0,
            &["Infantry"],
        ),
        owned_area_unit(
            2,
            "PlyrAmerica",
            "AmericaInfantryMissileDefender",
            9.0,
            6.0,
            &["Infantry"],
        ),
    ]);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    let mut ge_two = eval_player_unit_kind_in_area(3, 2, KINDOF_INFANTRY, "HoldZone");
    let mut eq_zero = eval_player_unit_kind_in_area(2, 0, KINDOF_INFANTRY, "HoldZone");
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut ge_two, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut eq_zero, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "kind == 0 must not fire at map load while infantry occupy the trigger"
    );
}

#[test]
fn live_skirmish_leftover_conditions_read_host_snapshot() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    use crate::scripting::engine::{
        ScriptOwnerQuery, ScriptPlayerAreaQuery, ScriptSkirmishCensus, ScriptSpecialPowerReadiness,
    };
    struct Owner;
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(0)
        }
        fn player_area_value(
            &self,
            player: &str,
            area: &str,
            _: Option<&str>,
            query: ScriptPlayerAreaQuery<'_>,
        ) -> ScriptOwnerQuery<(i32, bool)> {
            assert_eq!((player, area), ("PlyrAmerica", "HoldZone"));
            ScriptOwnerQuery::Present((
                if matches!(query, ScriptPlayerAreaQuery::Value) {
                    2000
                } else {
                    1
                },
                false,
            ))
        }
        fn skirmish_census(
            &self,
            player: &str,
            _: Option<&str>,
            _: ScriptSkirmishCensus,
        ) -> ScriptOwnerQuery<i32> {
            assert_eq!(player, "PlyrAmerica");
            ScriptOwnerQuery::Present(1)
        }
        fn skirmish_special_power_readiness(
            &self,
            player: &str,
            power: &str,
            _: Option<&str>,
        ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
            assert_eq!((player, power), ("PlyrAmerica", "SuperweaponDaisyCutter"));
            ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness {
                template_exists: true,
                ready: true,
                next_frame: 0,
            })
        }
    }
    let mut owner = Owner;
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

    let mut power = Condition::new(ConditionType::SkirmishSpecialPowerReady);
    power
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    power
        .add_parameter(Parameter::with_string(
            ParameterType::SpecialPower,
            "SuperweaponDaisyCutter".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut power, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_SPECIAL_POWER_READY must see host special_power_ready"
    );

    let mut value = Condition::new(ConditionType::SkirmishValueInArea);
    value
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    value
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    value
        .add_parameter(Parameter::with_int(ParameterType::Int, 2000))
        .unwrap();
    value
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut value, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_VALUE_IN_AREA must sum owner build_cost"
    );

    let mut unowned = Condition::new(ConditionType::SkirmishUnownedFactionUnitExists);
    unowned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    unowned
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    unowned
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut unowned, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS must count owner neutral unmanned"
    );

    let mut garrisoned = Condition::new(ConditionType::SkirmishPlayerHasComparisonGarrisoned);
    garrisoned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    garrisoned
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    garrisoned
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut garrisoned, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED must count owner garrisonable+occupied"
    );

    let mut captured = Condition::new(ConditionType::SkirmishPlayerHasComparisonCapturedUnits);
    captured
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    captured
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    captured
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut captured, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS must count owner captured"
    );

    let mut units = Condition::new(ConditionType::SkirmishPlayerHasUnitsInArea);
    units
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    units
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut units, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_UNITS_IN_AREA must see owner objects in the pad"
    );
}

#[test]
fn bridge_broken_reads_host_named_state_only_on_damage_edge() {
    use crate::scripting::engine::{ScriptBridgeStatus, ScriptExecutionDriver, ScriptOwnerQuery};
    // Dispatch contract; Main bridge_condition_owner_tests exercises actual
    // terrain state edges, frame gates, save/load and nonconsuming queries.
    struct BridgeOwner(ScriptOwnerQuery<ScriptBridgeStatus>);
    impl ScriptExecutionDriver for BridgeOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn bridge_status(
            &self,
            name: &str,
            _this_object: Option<u32>,
        ) -> ScriptOwnerQuery<ScriptBridgeStatus> {
            assert_eq!(name, "ConvoyBridge");
            self.0
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
    let mut broken = Condition::new(ConditionType::BridgeBroken);
    broken
        .add_parameter(Parameter::with_string(
            ParameterType::Bridge,
            "ConvoyBridge".into(),
        ))
        .unwrap();
    let mut repaired = Condition::new(ConditionType::BridgeRepaired);
    repaired
        .add_parameter(Parameter::with_string(
            ParameterType::Bridge,
            "ConvoyBridge".into(),
        ))
        .unwrap();
    let mut owner = BridgeOwner(ScriptOwnerQuery::Missing);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut broken, &mut owner)
            .unwrap(),
        ScriptConditionResult::False
    );
    for (status, broken_result, repaired_result) in [
        (
            ScriptBridgeStatus {
                broken: true,
                repaired: false,
            },
            ScriptConditionResult::True,
            ScriptConditionResult::False,
        ),
        (
            ScriptBridgeStatus {
                broken: false,
                repaired: false,
            },
            ScriptConditionResult::False,
            ScriptConditionResult::False,
        ),
        (
            ScriptBridgeStatus {
                broken: false,
                repaired: true,
            },
            ScriptConditionResult::False,
            ScriptConditionResult::True,
        ),
    ] {
        owner.0 = ScriptOwnerQuery::Present(status);
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut broken, &mut owner)
                .unwrap(),
            broken_result
        );
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut repaired, &mut owner)
                .unwrap(),
            repaired_result
        );
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut broken, &mut owner)
                .unwrap(),
            broken_result,
            "query does not consume the owner edge"
        );
    }
}

#[test]
fn player_set_give_money_use_only_the_borrowed_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    #[derive(Default)]
    struct MoneyOwner(Vec<HostScriptMoneyRequest>);
    impl ScriptExecutionDriver for MoneyOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn money(
            &mut self,
            request: &HostScriptMoneyRequest,
            _: Option<&str>,
        ) -> Option<GameLogicResult<()>> {
            self.0.push(request.clone());
            Some(Ok(()))
        }
    }
    let mut owner = MoneyOwner::default();
    let peer = MoneyOwner::default();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);

    let mut set = ScriptAction::new(ScriptActionType::PlayerSetMoney);
    set.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrAmerica".into(),
    ))
    .unwrap();
    set.add_parameter(Parameter::with_int(ParameterType::Int, 10_000))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&set, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut give = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    give.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrChina".into(),
    ))
    .unwrap();
    give.add_parameter(Parameter::with_int(ParameterType::Int, -500))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&give, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.0,
        vec![
            HostScriptMoneyRequest::Set {
                player: "PlyrAmerica".into(),
                amount: 10_000,
            },
            HostScriptMoneyRequest::Give {
                player: "PlyrChina".into(),
                amount: -500,
            },
        ]
    );
    assert!(
        peer.0.is_empty(),
        "a peer receives no broadcast cash mutation"
    );
}
#[test]
fn named_team_unmanned_stealth_radar_use_supplied_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut owner = ObjectEffectOwner::default();

    let mut named_unmanned = ScriptAction::new(ScriptActionType::NamedSetUnmannedStatus);
    named_unmanned
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "SnipedHumvee".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_unmanned, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_unmanned = ScriptAction::new(ScriptActionType::TeamSetUnmannedStatus);
    team_unmanned
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_unmanned, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let delete_all = ScriptAction::new(ScriptActionType::DeleteAllUnmanned);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&delete_all, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut named_stealth = ScriptAction::new(ScriptActionType::NamedSetStealthEnabled);
    named_stealth
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ColonelBurton".into(),
        ))
        .unwrap();
    named_stealth
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_stealth, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_stealth = ScriptAction::new(ScriptActionType::TeamSetStealthEnabled);
    team_stealth
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamChina".into(),
        ))
        .unwrap();
    team_stealth
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_stealth, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut object_radar = ScriptAction::new(ScriptActionType::ObjectCreateRadarEvent);
    object_radar
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    object_radar
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&object_radar, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team_radar = ScriptAction::new(ScriptActionType::TeamCreateRadarEvent);
    team_radar
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamGLA".into(),
        ))
        .unwrap();
    team_radar
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_radar, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.unmanned,
        vec![
            HostScriptUnmannedRequest::Named {
                unit: "SnipedHumvee".to_string()
            },
            HostScriptUnmannedRequest::Team {
                team: "teamAmerica".to_string()
            },
            HostScriptUnmannedRequest::DeleteAll,
        ]
    );
    assert_eq!(
        owner.stealth,
        vec![
            HostScriptStealthEnabledRequest::Named {
                unit: "ColonelBurton".to_string(),
                enabled: false,
            },
            HostScriptStealthEnabledRequest::Team {
                team: "teamChina".to_string(),
                enabled: true,
            },
        ]
    );
    assert_eq!(
        owner.radar,
        vec![
            HostScriptRadarEventRequest::Object {
                unit: "Hero".to_string(),
                event_type: 4,
            },
            HostScriptRadarEventRequest::Team {
                team: "teamGLA".to_string(),
                event_type: 3,
            },
        ]
    );
}

#[test]
fn live_host_team_reached_waypoints_end_uses_snapshot_labels() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    struct Owner;
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_waypoint_complete(
            &self,
            team: &str,
            path: &str,
            _: Option<u32>,
            _: Option<u32>,
        ) -> crate::scripting::engine::ScriptOwnerQuery<bool> {
            assert_eq!((team, path), ("USA_RangerSquad", "HeroPath"));
            crate::scripting::engine::ScriptOwnerQuery::Present(true)
        }
    }
    let mut owner = Owner;
    let mut condition = Condition::new(ConditionType::TeamReachedWaypointsEnd);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "HeroPath".into(),
        ))
        .unwrap();
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
}

#[test]
fn live_host_skirmish_discovered_uses_snapshot() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    struct Owner;
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_discovered_by(
            &self,
            player: &str,
            viewer: &str,
            _: Option<&str>,
        ) -> crate::scripting::engine::ScriptOwnerQuery<bool> {
            assert_eq!((player, viewer), ("PlyrChina", "PlyrAmerica"));
            crate::scripting::engine::ScriptOwnerQuery::Present(true)
        }
    }
    let mut owner = Owner;
    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasDiscoveredPlayer);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrChina".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::True
    );
}

#[test]
fn live_host_from_named_special_power_uses_host_id() {
    use crate::scripting::engine::{ScriptOwnerQuery, ScriptPlayerEventSource};

    struct EventOwner;
    impl ScriptExecutionDriver for EventOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_event_source(
            &self,
            player: &str,
            source: Option<&str>,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ScriptPlayerEventSource> {
            assert_eq!(player, "PlyrAmerica");
            assert_eq!(source, Some("ParticleCannon"));
            ScriptOwnerQuery::Present(ScriptPlayerEventSource {
                player_index: 1,
                source_object: 42,
            })
        }
    }

    let engine = ScriptEngine::new().expect("owned script engine");
    engine.notify_of_triggered_special_power(1, "SuperweaponParticleUplinkCannon", 42);
    engine.notify_of_triggered_special_power(1, "SuperweaponParticleUplinkCannon", 43);
    let mut condition = Condition::new(ConditionType::PlayerTriggeredSpecialPowerFromNamed);
    for (parameter_type, value) in [
        (ParameterType::Side, "PlyrAmerica"),
        (
            ParameterType::SpecialPower,
            "SuperweaponParticleUplinkCannon",
        ),
        (ParameterType::Unit, "ParticleCannon"),
    ] {
        condition
            .add_parameter(Parameter::with_string(parameter_type, value.into()))
            .unwrap();
    }
    let evaluator_state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &evaluator_state);
    let mut driver = EventOwner;
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::True,
        "FROM_NAMED uses the owning driver's host ID without a Core registry or tracker"
    );
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::False
    );
    assert!(engine.is_special_power_triggered(1, "SuperweaponParticleUplinkCannon", false, 43));
}

#[test]
fn skirmish_approach_and_defense_host_queues_roundtrip() {
    let engine = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);
    let mut owner = OwnedAiActions::default();
    let mut approach = ScriptAction::new(ScriptActionType::SkirmishFollowApproachPath);
    approach
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    approach
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "ApproachPath".into(),
        ))
        .unwrap();
    approach
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&approach, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    let mut defense = ScriptAction::new(ScriptActionType::SkirmishBuildStructureFlank);
    defense
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaPatriotBattery".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&defense, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.effects.len(), 2);
    assert_eq!(
        owner.effects[0],
        (
            "approach".into(),
            vec![
                "teamAmerica".into(),
                "ApproachPath".into(),
                "true".into(),
                "true".into()
            ]
        )
    );
    assert_eq!(owner.effects[1].0, "defense");
    assert_eq!(
        &owner.effects[1].1[1..],
        &["AmericaPatriotBattery".to_string(), "true".to_string()]
    );
}

#[test]
fn set_base_construction_speed_queues_host() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut owner = OwnedAiActions::default();
    let mut action = ScriptAction::new(ScriptActionType::SetBaseConstructionSpeed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.effects,
        vec![("delay".into(), vec!["PlyrAmerica".into(), "3".into()])]
    );
}

#[test]
fn set_train_held_uses_supplied_owner_without_request_drain() {
    use crate::scripting::engine::ScriptExecutionDriver;
    struct Owner(Option<(String, bool)>);
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn set_named_train_held(&mut self, name: &str, value: bool) -> Option<()> {
            self.0 = Some((name.into(), value));
            Some(())
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(42));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::SetTrainHeld);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "CivilianTrain".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut owner = Owner(None);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.0, Some(("CivilianTrain".into(), true)));
}

#[test]
fn nearest_command_buttons_use_supplied_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    #[derive(Default)]
    struct NearestOwner(Vec<HostScriptUseCommandButtonRequest>);
    impl crate::scripting::engine::ScriptExecutionDriver for NearestOwner {
        fn team_nearest_button(
            &mut self,
            request: &HostScriptUseCommandButtonRequest,
            _: Option<crate::team::TeamID>,
            _: Option<crate::team::TeamID>,
        ) -> Option<crate::GameLogicResult<()>> {
            self.0.push(request.clone());
            Some(Ok(()))
        }
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
    }
    let mut owner = NearestOwner::default();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);

    let mut nearest =
        ScriptAction::new(ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit);
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Stop".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&nearest, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut kindof = ScriptAction::new(ScriptActionType::TeamAllUseCommandbuttonOnNearestKindof);
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Hijack".into(),
        ))
        .unwrap();
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::KindOfParam,
            "VEHICLE".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&kindof, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let queued = owner.0;
    assert!(
        queued.contains(&HostScriptUseCommandButtonRequest::TeamOnNearestEnemy {
            team: "teamAmerica".into(),
            button: "Command_Stop".into(),
        })
    );
    assert!(
        queued.contains(&HostScriptUseCommandButtonRequest::TeamOnNearestKindof {
            team: "teamAmerica".into(),
            button: "Command_Hijack".into(),
            kindof: "VEHICLE".into(),
        })
    );
}

#[test]
fn idle_and_guard_for_framecount_use_borrowed_commands_and_preserve_pending_frames() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut owner = super::borrowed_order_owner::OrderOwner::default();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);

    let mut named_idle = ScriptAction::new(ScriptActionType::UnitIdleForFramecount);
    named_idle
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedRanger".into(),
        ))
        .unwrap();
    named_idle
        .add_parameter(Parameter::with_int(ParameterType::Int, 9))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_idle, &mut owner)
            .unwrap(),
        ScriptActionResult::Pending(9.0)
    );

    let mut named_guard = ScriptAction::new(ScriptActionType::UnitGuardForFramecount);
    named_guard
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedRanger".into(),
        ))
        .unwrap();
    named_guard
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_guard, &mut owner)
            .unwrap(),
        ScriptActionResult::Pending(4.0)
    );

    let mut team_idle = ScriptAction::new(ScriptActionType::TeamIdleForFramecount);
    team_idle
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    team_idle
        .add_parameter(Parameter::with_int(ParameterType::Int, 12))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_idle, &mut owner)
            .unwrap(),
        ScriptActionResult::Pending(12.0)
    );

    let mut team_guard = ScriptAction::new(ScriptActionType::TeamGuardForFramecount);
    team_guard
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    team_guard
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team_guard, &mut owner)
            .unwrap(),
        ScriptActionResult::Pending(3.0)
    );

    assert_eq!(
        owner.named,
        vec![
            ("Stop".into(), "NamedRanger".into()),
            ("Guard".into(), "NamedRanger".into())
        ]
    );
    assert_eq!(owner.idle, vec!["USA_RangerSquad", "USA_RangerSquad"]);
    owner.missing = true;
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named_guard, &mut owner)
            .unwrap(),
        ScriptActionResult::Success,
        "CPP missing object does not start a timer"
    );
}

#[test]
fn move_towards_nearest_uses_borrowed_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut owner = super::borrowed_order_owner::OrderOwner::default();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);

    let mut named = ScriptAction::new(ScriptActionType::UnitMoveTowardsNearestObjectType);
    named
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedScout".into(),
        ))
        .unwrap();
    named
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaCommandCenter".into(),
        ))
        .unwrap();
    named
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&named, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    let mut team = ScriptAction::new(ScriptActionType::TeamMoveTowardsNearestObjectType);
    team.add_parameter(Parameter::with_string(
        ParameterType::Team,
        "USA_Scout".into(),
    ))
    .unwrap();
    team.add_parameter(Parameter::with_string(
        ParameterType::ObjectType,
        "AmericaCommandCenter".into(),
    ))
    .unwrap();
    team.add_parameter(Parameter::with_string(
        ParameterType::TriggerArea,
        "HoldZone".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&team, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        owner.nearest,
        vec![
            HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
                unit: "NamedScout".into(),
                object_type: "AmericaCommandCenter".into(),
                trigger: "HoldZone".into(),
            },
            HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
                team: "USA_Scout".into(),
                object_type: "AmericaCommandCenter".into(),
                trigger: "HoldZone".into(),
            },
        ]
    );
}

#[test]
fn team_wait_for_not_contained_uses_supplied_roster_without_ambient_census() {
    use crate::scripting::engine::{
        ScriptExecutionDriver, ScriptOwnerQuery, script_team_is_contained,
    };
    struct Owner(Vec<Option<bool>>);
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn team_contained(
            &self,
            name: &str,
            _: Option<u32>,
            _: Option<u32>,
            all: bool,
        ) -> ScriptOwnerQuery<bool> {
            if name != "OwnedTeam" {
                return ScriptOwnerQuery::Missing;
            }
            ScriptOwnerQuery::Present(script_team_is_contained(self.0.iter().copied(), all))
        }
    }
    let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(42));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = Owner(vec![None, Some(true)]);
    for (members, all_expected, partial_expected) in [
        (vec![None, Some(true)], true, true),
        (vec![Some(true), Some(false)], false, true),
        (vec![Some(false)], false, false),
        (vec![None], false, false),
    ] {
        owner.0 = members;
        for (kind, pending) in [
            (ScriptActionType::TeamWaitForNotContainedAll, all_expected),
            (
                ScriptActionType::TeamWaitForNotContainedPartial,
                partial_expected,
            ),
        ] {
            let mut action = ScriptAction::new(kind);
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Team,
                    "OwnedTeam".into(),
                ))
                .unwrap();
            assert_eq!(
                dispatcher
                    .execute_action_with_driver(&action, &mut owner)
                    .unwrap(),
                if pending {
                    ScriptActionResult::Pending(1.0)
                } else {
                    ScriptActionResult::Success
                }
            );
        }
    }
}

#[test]
fn named_owned_by_player_uses_explicit_owner_without_ambient_fallback() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
    struct Owner;
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_owned_by_player(
            &self,
            unit: &str,
            player: &str,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<bool> {
            if unit != "OwnedUnit" || !["Owner", "Peer"].contains(&player) {
                return ScriptOwnerQuery::Missing;
            }
            ScriptOwnerQuery::Present(player == "Owner")
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    for (unit, player, expected) in [
        ("OwnedUnit", "Owner", true),
        ("OwnedUnit", "Peer", false),
        ("MissingUnit", "Owner", false),
        ("OwnedUnit", "MissingPlayer", false),
    ] {
        let mut condition = Condition::new(ConditionType::NamedOwnedByPlayer);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Unit, unit.into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
            .unwrap();
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut Owner)
                .unwrap(),
            if expected {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            }
        );
    }
}

#[derive(Default)]
struct ObjectEffectOwner {
    sound: Vec<HostScriptObjectSoundRequest>,
    unmanned: Vec<HostScriptUnmannedRequest>,
    stealth: Vec<HostScriptStealthEnabledRequest>,
    radar: Vec<HostScriptRadarEventRequest>,
}
impl crate::scripting::engine::ScriptExecutionDriver for ObjectEffectOwner {
    fn after_action(&mut self) -> crate::GameLogicResult<()> {
        Ok(())
    }
    fn object_effect(
        &mut self,
        request: crate::scripting::engine::ScriptObjectEffect<'_>,
        _object: Option<u32>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<crate::GameLogicResult<()>> {
        use crate::scripting::engine::ScriptObjectEffect;
        match request {
            ScriptObjectEffect::Sound(value) => self.sound.push(value.clone()),
            ScriptObjectEffect::Unmanned(value) => self.unmanned.push(value.clone()),
            ScriptObjectEffect::Stealth(value) => self.stealth.push(value.clone()),
            ScriptObjectEffect::Radar(value) => self.radar.push(value.clone()),
        }
        Some(Ok(()))
    }
}

#[derive(Default)]
struct LifecycleOwner {
    effects: Vec<HostScriptKillDeleteDamageRequest>,
    players: Vec<HostScriptPlayerMiscRequest>,
}
impl crate::scripting::engine::ScriptExecutionDriver for LifecycleOwner {
    fn after_action(&mut self) -> crate::GameLogicResult<()> {
        Ok(())
    }
    fn delete_or_kill(
        &mut self,
        request: &HostScriptKillDeleteDamageRequest,
        _object: Option<u32>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<crate::GameLogicResult<()>> {
        self.effects.push(request.clone());
        Some(Ok(()))
    }
    fn named_damage(
        &mut self,
        name: &str,
        amount: i32,
        _this_object: Option<u32>,
    ) -> Option<crate::GameLogicResult<()>> {
        self.effects
            .push(HostScriptKillDeleteDamageRequest::NamedDamage {
                unit: name.into(),
                amount,
            });
        Some(Ok(()))
    }
    fn team_damage(
        &mut self,
        name: &str,
        amount: f32,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<crate::GameLogicResult<()>> {
        self.effects
            .push(HostScriptKillDeleteDamageRequest::TeamDamage {
                team: name.into(),
                amount,
            });
        Some(Ok(()))
    }
    fn kill_player(
        &mut self,
        name: &str,
        _current: Option<&str>,
    ) -> Option<crate::GameLogicResult<()>> {
        self.players.push(HostScriptPlayerMiscRequest::Kill {
            player: name.into(),
        });
        Some(Ok(()))
    }
}

#[test]
fn partial_command_button_uses_supplied_owner_ingress() {
    struct Owner {
        received: Option<(String, String, f32)>,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_partial_button(
            &mut self,
            team: &str,
            button: &str,
            percentage: f32,
            _: Option<u32>,
            _: Option<u32>,
        ) -> Option<GameLogicResult<()>> {
            self.received = Some((team.into(), button.into(), percentage));
            Some(Ok(()))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);
    let mut owner = Owner { received: None };
    let mut action = ScriptAction::new(ScriptActionType::TeamPartialUseCommandbutton);
    action
        .add_parameter(Parameter::with_real(ParameterType::Real, 50.0))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Stop".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.received,
        Some(("teamAmerica".into(), "Command_Stop".into(), 50.0))
    );
}
