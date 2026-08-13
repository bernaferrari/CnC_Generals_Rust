use super::*;
use crate::scripting::executor::{ScriptActionDispatcher, ScriptActionResult, ScriptContext};

fn always_true_condition() -> Box<OrCondition> {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    Box::new(condition)
}

fn set_flag_action(flag_name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            flag_name.to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    Box::new(action)
}

fn call_subroutine_action(name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::CallSubroutine);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            name.to_string(),
        ))
        .unwrap();
    Box::new(action)
}

fn subroutine(name: &str, action: Box<ScriptAction>) -> Script {
    let mut script = Script::new();
    script.script_name = name.to_string();
    script.is_subroutine = true;
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = Some(action);
    script
}

#[derive(Debug)]
struct IdleSequentialAi;

impl crate::modules::AIUpdateInterface for IdleSequentialAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_moving(&self) -> bool {
        false
    }

    fn is_idle(&self) -> bool {
        true
    }

    fn set_movement_target(&mut self, _target: &crate::common::Coord3D) -> Result<(), String> {
        Ok(())
    }
}

struct RegisteredSequentialObjectCleanup(u32);

impl Drop for RegisteredSequentialObjectCleanup {
    fn drop(&mut self) {
        OBJECT_REGISTRY.unregister_object(self.0);
    }
}

#[test]
fn test_script_engine_creation() {
    let engine = ScriptEngine::new().unwrap();
    // Slot 0 is reserved, matching runtime reset semantics.
    assert_eq!(engine.with_inner(|i| i.num_counters), 1);
    assert_eq!(engine.with_inner(|i| i.num_flags), 1);
    assert_eq!(engine.with_inner(|i| i.fade), TFade::None);
    assert!(!engine.is_game_ending());
}

#[test]
fn test_counter_operations() {
    let mut engine = ScriptEngine::new().unwrap();

    // Set a counter
    engine.set_counter("test_counter", 42).unwrap();
    assert_eq!(engine.with_inner(|i| i.num_counters), 2);

    // Get the counter
    let counter = engine.get_counter("test_counter").unwrap();
    assert_eq!(counter.value, 42);
    assert_eq!(counter.name, "test_counter");
}

#[test]
fn test_flag_operations() {
    let mut engine = ScriptEngine::new().unwrap();

    // Set a flag
    engine.set_flag("test_flag", true).unwrap();
    assert_eq!(engine.with_inner(|i| i.num_flags), 2);

    // Get the flag
    let flag = engine.get_flag("test_flag").unwrap();
    assert!(flag.value);
    assert_eq!(flag.name, "test_flag");
}

#[test]
fn test_end_game_timer() {
    let engine = ScriptEngine::new().unwrap();
    assert!(!engine.is_game_ending());

    engine.start_end_game_timer();
    assert!(engine.is_game_ending());
    assert_eq!(engine.with_inner(|i| i.end_game_timer), 120);

    engine.start_quick_end_game_timer();
    assert_eq!(engine.with_inner(|i| i.end_game_timer), 1);

    engine.start_close_window_timer();
    assert_eq!(engine.with_inner(|i| i.close_window_timer), 120);
}

#[test]
fn test_time_freeze() {
    let mut engine = ScriptEngine::new().unwrap();
    assert!(!engine.is_time_frozen_script());

    engine.do_freeze_time();
    assert!(engine.is_time_frozen_script());

    engine.do_unfreeze_time();
    assert!(!engine.is_time_frozen_script());
}

#[test]
fn test_debug_freeze_stops_update_progression() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_timer("freeze_timer", 10).unwrap();
    engine.set_time_frozen_debug(true);

    engine
        .update()
        .expect("update should succeed while debug-frozen");
    let frozen_counter = engine.get_counter("freeze_timer").unwrap();
    assert_eq!(
        frozen_counter.value, 10,
        "debug freeze should prevent countdown-timer advancement"
    );

    engine.set_time_frozen_debug(false);
    engine
        .update()
        .expect("update should succeed when debug freeze is cleared");
    let resumed_counter = engine.get_counter("freeze_timer").unwrap();
    assert_eq!(
        resumed_counter.value, 9,
        "timer should resume once debug freeze is cleared"
    );
}

#[test]
fn pending_resume_frame_is_next_frame_for_single_frame_wait() {
    assert_eq!(ScriptEngine::pending_resume_frame(100, 1.0), 101);
    assert_eq!(ScriptEngine::pending_resume_frame(100, 0.0), 101);
}

#[test]
fn sequential_pending_wait_conversion_matches_cxx_wait_patterns() {
    // Retry-style wait actions re-run the same instruction on the next frame.
    assert_eq!(
        ScriptEngine::pending_to_sequential_wait_frames(1.0, true),
        0
    );
    assert_eq!(
        ScriptEngine::pending_to_sequential_wait_frames(3.0, true),
        2
    );

    // Framecount actions wait before advancing to the next instruction.
    assert_eq!(
        ScriptEngine::pending_to_sequential_wait_frames(1.0, false),
        1
    );
    assert_eq!(
        ScriptEngine::pending_to_sequential_wait_frames(3.0, false),
        3
    );
}

#[test]
fn sequential_pending_retry_action_classification_matches_cxx() {
    assert!(
        ScriptEngine::pending_repeats_current_sequential_instruction(
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
        )
    );
    assert!(
        ScriptEngine::pending_repeats_current_sequential_instruction(
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
        )
    );
    assert!(
        ScriptEngine::pending_repeats_current_sequential_instruction(
            ScriptActionType::TeamWaitForNotContainedAll
        )
    );
    assert!(
        ScriptEngine::pending_repeats_current_sequential_instruction(
            ScriptActionType::TeamWaitForNotContainedPartial
        )
    );
    assert!(
        !ScriptEngine::pending_repeats_current_sequential_instruction(
            ScriptActionType::TeamGuardForFramecount
        )
    );

    assert!(ScriptEngine::pending_is_sequential_only_action(
        ScriptActionType::TeamGuardForFramecount
    ));
    assert!(ScriptEngine::pending_is_sequential_only_action(
        ScriptActionType::TeamWaitForNotContainedAll
    ));
    assert!(!ScriptEngine::pending_is_sequential_only_action(
        ScriptActionType::TeamGuardPosition
    ));
}

#[test]
fn set_script_list_initializes_delay_evaluation_frame_offset() {
    let mut engine = ScriptEngine::new().unwrap();

    let mut delayed_script = Script::new();
    delayed_script.delay_evaluation_seconds = 3;
    delayed_script.frame_to_evaluate_at = 9999;

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(delayed_script));

    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    let frame = engine
        .with_inner(|i| {
            i.side_script_lists[0]
                .as_ref()
                .and_then(|list| list.first_script.as_ref())
                .map(|s| s.frame_to_evaluate_at)
        })
        .expect("script should be present");
    assert!(frame <= (2 * LOGICFRAMES_PER_SECOND as u32));
}

#[test]
fn set_script_list_infers_condition_team_name_for_singleton_team() {
    let mut engine = ScriptEngine::new().unwrap();
    let team_name = "RuntimeInitSingletonTeam".to_string();

    if let Ok(mut factory) = get_team_factory().lock() {
        let _ = factory.init_team(team_name.clone().into(), "PlyrCivilian".into(), true, None);
    }

    let mut team_param = Parameter::new(ParameterType::Team);
    team_param.string_value = team_name.clone();
    team_param.initialized = true;

    let mut condition = Condition::new(ConditionType::ConditionTrue);
    condition.add_parameter(team_param).unwrap();

    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(condition)));

    let mut script = Script::new();
    script.condition = Some(Box::new(or_condition));
    script.condition_team_name.clear();

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(script));

    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    let condition_team = engine
        .with_inner(|i| {
            i.side_script_lists[0]
                .as_ref()
                .and_then(|list| list.first_script.as_ref())
                .map(|s| s.condition_team_name.clone())
        })
        .expect("script should be present");
    assert_eq!(condition_team, team_name);
}

#[test]
fn call_subroutine_executes_in_place_and_persists_one_shot_state() {
    let mut engine = ScriptEngine::new().unwrap();

    let mut condition = Condition::new(ConditionType::ConditionTrue);
    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(condition)));

    let mut subroutine = Script::new();
    subroutine.script_name = "SubroutinePersist".to_string();
    subroutine.is_subroutine = true;
    subroutine.is_one_shot = true;
    subroutine.condition = Some(Box::new(or_condition));

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(subroutine));
    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    assert!(engine
        .execute_subroutine_by_name("SubroutinePersist")
        .unwrap());

    let is_active = engine
        .with_inner(|i| {
            i.side_script_lists[0]
                .as_ref()
                .and_then(|list| list.first_script.as_ref())
                .map(|s| s.is_active)
        })
        .expect("subroutine should still exist");
    assert!(!is_active);
}

#[test]
fn call_subroutine_resolves_subroutine_group_name_first() {
    let mut engine = ScriptEngine::new().unwrap();

    let mut condition = Condition::new(ConditionType::ConditionTrue);
    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(condition)));

    let mut grouped_subroutine = Script::new();
    grouped_subroutine.script_name = "InnerGroupedSubroutine".to_string();
    grouped_subroutine.is_subroutine = false;
    grouped_subroutine.is_one_shot = true;
    grouped_subroutine.condition = Some(Box::new(or_condition));

    let mut group = ScriptGroup::new();
    group.group_name = "NamedSubroutineGroup".to_string();
    group.is_group_subroutine = true;
    group.is_group_active = true;
    group.first_script = Some(Box::new(grouped_subroutine));

    let mut script_list = ScriptList::new();
    script_list.first_group = Some(Box::new(group));
    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    assert!(engine
        .execute_subroutine_by_name("NamedSubroutineGroup")
        .unwrap());

    let grouped_active = engine
        .with_inner(|i| {
            i.side_script_lists[0]
                .as_ref()
                .and_then(|list| list.first_group.as_ref())
                .and_then(|grp| grp.first_script.as_ref())
                .map(|s| s.is_active)
        })
        .expect("grouped subroutine should exist");
    assert!(!grouped_active);
}

#[test]
fn enable_disable_script_by_group_name_toggles_cxx_group_state() {
    // C++ ScriptEngine::enableScript/disableScript (ScriptEngine.cpp:6797-6823)
    // first-classifies a matching ScriptGroup separately from matching Script.
    // An inactive subroutine group must become callable immediately when enabled
    // and must stop being callable when disabled.
    let mut engine = ScriptEngine::new().unwrap();

    let mut member = Script::new();
    member.script_name = "Toggle Group Member".to_string();
    member.condition = Some(always_true_condition());
    member.action = Some(set_flag_action("group_toggle_fired"));

    let mut group = ScriptGroup::new();
    group.group_name = "Toggle Group".to_string();
    group.is_group_subroutine = true;
    group.is_group_active = false;
    group.first_script = Some(Box::new(member));

    let mut list = ScriptList::new();
    list.first_group = Some(Box::new(group));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    assert!(engine.set_script_active_by_name("Toggle Group", true));
    assert!(engine.execute_subroutine_by_name("Toggle Group").unwrap());
    assert!(engine.get_flag("group_toggle_fired").unwrap().value);

    engine.set_flag("group_toggle_fired", false).unwrap();
    assert!(engine.set_script_active_by_name("Toggle Group", false));
    assert!(engine.execute_subroutine_by_name("Toggle Group").unwrap());
    assert!(
        !engine.get_flag("group_toggle_fired").unwrap().value,
        "disabled groups must not execute through CALL_SUBROUTINE"
    );
}

#[test]
fn millisecond_script_seconds_use_ceil_frame_conversion() {
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_timer_millisecond_script_seconds("shell_camera", 0.12)
        .unwrap();

    let index = engine.allocate_counter("shell_camera").unwrap();
    let counter = engine
        .with_inner(|i| i.counters[index].clone())
        .expect("timer counter");
    assert_eq!(counter.value, 4);
    assert!(counter.is_countdown_timer);
}

#[test]
fn stop_timer_preserves_remaining_value() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_timer("pause_test", 17).unwrap();
    engine.stop_timer("pause_test").unwrap();

    let index = engine.allocate_counter("pause_test").unwrap();
    let counter = engine
        .with_inner(|i| i.counters[index].clone())
        .expect("pause counter");
    assert_eq!(counter.value, 17);
    assert!(!counter.is_countdown_timer);
}

#[test]
fn subtract_millisecond_script_seconds_can_drive_timer_negative() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_timer("negative_test", 2).unwrap();
    engine
        .subtract_from_timer_millisecond_script_seconds("negative_test", 0.12)
        .unwrap();

    let index = engine.allocate_counter("negative_test").unwrap();
    let counter = engine
        .with_inner(|i| i.counters[index].clone())
        .expect("negative counter");
    assert_eq!(counter.value, -2);
}

#[test]
fn vtune_enable_parity_state_round_trips_through_script_engine() {
    let _lock = crate::test_sync::lock();
    set_enable_vtune(false);

    let mut engine = ScriptEngine::new().unwrap();
    assert!(!engine.get_enable_vtune());
    engine.set_enable_vtune(true);
    assert!(engine.get_enable_vtune());
    engine.set_enable_vtune(false);
    assert!(!engine.get_enable_vtune());
}

#[test]
fn skate_override_parity_state_matches_cxx_delta_steps() {
    let _lock = crate::test_sync::lock();
    set_skate_distance_override(0.0);

    let mut engine = ScriptEngine::new().unwrap();
    let up = engine.adjust_skate_distance_override(0.25);
    assert!((up - 0.25).abs() < f32::EPSILON);
    assert!((engine.get_skate_distance_override() - 0.25).abs() < f32::EPSILON);

    let down = engine.adjust_skate_distance_override(-0.25);
    assert!(down.abs() < f32::EPSILON);
    assert!(engine.get_skate_distance_override().abs() < f32::EPSILON);
}

/// C++ `ScriptEngine::executeActions` handles `CALL_SUBROUTINE` by calling
/// `callSubroutine` → `executeScript` **immediately**, then continues the
/// remaining outer actions. Nested `with_script_engine_mut` must not panic
/// and must observe outer mutations already applied.
#[test]
fn nested_with_script_engine_mut_runs_immediately_like_cxx_call_subroutine() {
    let engine = ScriptEngine::new().unwrap();
    engine.with_active(|| {
        engine.set_counter("outer_before", 1).unwrap();
        let mut order = Vec::new();
        order.push("outer_before");

        let nested_saw_outer = with_active_script_engine_mut(|nested| {
            nested.set_counter("nested", 2).unwrap();
            nested.set_flag("nested_flag", true).unwrap();
            order.push("nested");
            nested.get_counter("outer_before").map(|c| c.value)
        });

        order.push("outer_after");
        assert_eq!(nested_saw_outer, Some(Some(1)));
        assert_eq!(engine.get_counter("nested").unwrap().value, 2);
        assert!(engine.get_flag("nested_flag").unwrap().value);
        assert_eq!(
            order,
            ["outer_before", "nested", "outer_after"],
            "nested CALL_SUBROUTINE-style work runs immediately, then outer continues"
        );
    });
}

#[test]
fn with_script_engine_mut_nested_from_global_runs_immediately() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();

    let order = std::sync::Mutex::new(Vec::new());
    let result = with_script_engine_mut(|engine| {
        engine.set_counter("g_outer", 10).unwrap();
        order.lock().unwrap().push("outer");
        let nested = with_script_engine_mut(|engine| {
            order.lock().unwrap().push("nested");
            engine.set_counter("g_nested", 20).unwrap();
            engine.get_counter("g_outer").map(|c| c.value)
        });
        order.lock().unwrap().push("after");
        (nested, engine.get_counter("g_nested").map(|c| c.value))
    });

    assert_eq!(result, Some((Some(Some(10)), Some(20))));
    assert_eq!(*order.lock().unwrap(), ["outer", "nested", "after"]);
}

/// Campaign scene-control actions run while `ScriptEngine::update` owns the
/// global engine handle.  They must therefore mutate the lexically active
/// engine, rather than trying to take that handle again.
#[test]
fn active_script_campaign_scene_actions_do_not_relock_the_global_engine() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let completed = with_script_engine_mut(|engine| {
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let reveal_name = "ActiveSceneControlReveal";
        let mut reveal = ScriptAction::new(ScriptActionType::MapRevealPermanentlyAtWaypoint);
        reveal
            .add_parameter(Parameter::with_string(
                ParameterType::Waypoint,
                "MissingWaypointIsStillTracked".to_string(),
            ))
            .expect("waypoint parameter");
        reveal
            .add_parameter(Parameter::with_real(ParameterType::Real, 128.0))
            .expect("radius parameter");
        reveal
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "MissingPlayerIsStillTracked".to_string(),
            ))
            .expect("player parameter");
        reveal
            .add_parameter(Parameter::with_string(
                ParameterType::Revealname,
                reveal_name.to_string(),
            ))
            .expect("reveal-name parameter");
        assert_eq!(
            dispatcher.execute_action(&reveal).expect("reveal action"),
            ScriptActionResult::Success
        );
        assert!(
            engine.with_inner(|inner| inner
                .named_reveals
                .iter()
                .any(|entry| entry.reveal_name == reveal_name)),
            "C++ creates the named reveal before trying to reveal its waypoint"
        );

        let freeze = ScriptAction::new(ScriptActionType::FreezeTime);
        assert_eq!(
            dispatcher.execute_action(&freeze).expect("freeze action"),
            ScriptActionResult::Success
        );
        assert!(engine.is_time_frozen_script());

        let unfreeze = ScriptAction::new(ScriptActionType::UnfreezeTime);
        assert_eq!(
            dispatcher
                .execute_action(&unfreeze)
                .expect("unfreeze action"),
            ScriptActionResult::Success
        );
        assert!(!engine.is_time_frozen_script());

        let mut sway = ScriptAction::new(ScriptActionType::SetTreeSway);
        for parameter in [
            Parameter::with_real(ParameterType::Real, 0.5),
            Parameter::with_real(ParameterType::Real, 0.25),
            Parameter::with_real(ParameterType::Real, 0.1),
            Parameter::with_int(ParameterType::Int, 40),
            Parameter::with_real(ParameterType::Real, 0.75),
        ] {
            sway.add_parameter(parameter).expect("tree-sway parameter");
        }
        assert_eq!(
            dispatcher.execute_action(&sway).expect("tree-sway action"),
            ScriptActionResult::Success
        );
        let breeze = engine.get_breeze_info();
        assert!((breeze.direction - 0.5).abs() < f32::EPSILON);
        assert!((breeze.intensity - 0.25).abs() < f32::EPSILON);
        assert_eq!(breeze.breeze_period, 40);

        let mut undo_reveal =
            ScriptAction::new(ScriptActionType::MapUndoRevealPermanentlyAtWaypoint);
        undo_reveal
            .add_parameter(Parameter::with_string(
                ParameterType::Revealname,
                reveal_name.to_string(),
            ))
            .expect("reveal-name parameter");
        assert_eq!(
            dispatcher
                .execute_action(&undo_reveal)
                .expect("undo reveal action"),
            ScriptActionResult::Success
        );
        assert!(engine.with_inner(|inner| inner
            .named_reveals
            .iter()
            .all(|entry| entry.reveal_name != reveal_name)));
    });

    assert_eq!(completed, Some(()));
}

#[test]
fn call_subroutine_via_nested_with_script_engine_mut_sets_flag_immediately() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();

    let mut condition = Condition::new(ConditionType::ConditionTrue);
    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(condition)));

    let mut set_flag = ScriptAction::new(ScriptActionType::SetFlag);
    set_flag
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "sub_ran".to_string(),
        ))
        .unwrap();
    set_flag
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();

    let mut subroutine = Script::new();
    subroutine.script_name = "ImmediateSub".to_string();
    subroutine.is_subroutine = true;
    subroutine.is_one_shot = true;
    subroutine.condition = Some(Box::new(or_condition));
    subroutine.action = Some(Box::new(set_flag));

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(subroutine));

    let found = with_script_engine_mut(|engine| {
        engine
            .set_script_list_for_player(0, Some(Box::new(script_list)))
            .unwrap();
        engine.set_counter("before_sub", 1).unwrap();
        let found = engine.execute_subroutine_by_name("ImmediateSub").unwrap();
        // Nested SET_FLAG must already be visible (C++ immediate order).
        let flag = engine.get_flag("sub_ran").map(|f| f.value).unwrap_or(false);
        engine.set_counter("after_sub", 2).unwrap();
        (found, flag, engine.get_counter("before_sub").unwrap().value)
    });

    assert_eq!(found, Some((true, true, 1)));
    let after = with_script_engine_ref(|engine| engine.get_counter("after_sub").map(|c| c.value));
    assert_eq!(after, Some(Some(2)));
}

#[test]
fn nested_call_subroutine_reenters_immediately_through_scoped_tls() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();

    let mut always_true = OrCondition::new();
    always_true
        .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));

    let mut mark_inner = ScriptAction::new(ScriptActionType::SetFlag);
    mark_inner
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "nested_call_subroutine_ran".to_string(),
        ))
        .unwrap();
    mark_inner
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();

    let mut inner = Script::new();
    inner.script_name = "ScopedTlsInner".to_string();
    inner.is_subroutine = true;
    inner.is_one_shot = true;
    inner.condition = Some(Box::new(always_true));
    inner.action = Some(Box::new(mark_inner));

    let mut call_inner = ScriptAction::new(ScriptActionType::CallSubroutine);
    call_inner
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "ScopedTlsInner".to_string(),
        ))
        .unwrap();

    let mut outer = Script::new();
    outer.script_name = "ScopedTlsOuter".to_string();
    outer.is_subroutine = true;
    outer.is_one_shot = true;
    outer.condition = Some(Box::new({
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        condition
    }));
    outer.action = Some(Box::new(call_inner));

    let mut list = ScriptList::new();
    list.append_script(Box::new(outer));
    list.append_script(Box::new(inner));

    let result = with_script_engine_mut(|engine| {
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        let found = engine.execute_subroutine_by_name("ScopedTlsOuter").unwrap();
        let nested_flag = engine
            .get_flag("nested_call_subroutine_ran")
            .map(|flag| flag.value);
        (found, nested_flag)
    });

    assert_eq!(
        result,
        Some((true, Some(true))),
        "C++ CALL_SUBROUTINE must execute its callee before the outer action returns"
    );
}

#[test]
fn sequential_script_dispatch_reenters_immediately_without_holding_inner_borrow() {
    let _lock = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    let object_id = 0x51_5E_0001;
    let object = Arc::new(RwLock::new(crate::object::Object::new_test(
        object_id, 100.0,
    )));
    let ai: Arc<Mutex<dyn crate::modules::AIUpdateInterface>> =
        Arc::new(Mutex::new(IdleSequentialAi));
    object.write().unwrap().set_ai_update_interface(Some(ai));
    OBJECT_REGISTRY.register_object(object_id, &object);
    let _cleanup = RegisteredSequentialObjectCleanup(object_id);

    let mut list = ScriptList::new();
    list.append_script(Box::new(subroutine(
        "SequentialImmediateSubroutine",
        set_flag_action("sequential_nested_action_ran"),
    )));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    let mut call = Script::new();
    call.script_name = "SequentialCaller".to_string();
    call.action = Some(call_subroutine_action("SequentialImmediateSubroutine"));

    let mut sequence = SequentialScript::new();
    sequence.object_id = object_id;
    sequence.script_to_execute_sequentially = Some(Box::new(call));
    engine.append_sequential_script(sequence);

    engine
        .with_active(|| engine.evaluate_and_progress_all_sequential_scripts())
        .expect("sequential CALL_SUBROUTINE should execute immediately");

    assert!(
        engine
            .get_flag("sequential_nested_action_ran")
            .is_some_and(|flag| flag.value),
        "the nested SET_FLAG must run before the sequential action returns"
    );
    assert!(
        !engine.has_active_sequential_script_for_object(object_id),
        "an idle one-action sequence must advance through completion in this frame"
    );
}

#[test]
fn nested_call_subroutine_finds_callee_on_a_different_side_in_cxx_order() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();

    {
        let global = get_script_engine();
        let mut engine = global.write().unwrap();
        engine
            .as_mut()
            .expect("global ScriptEngine must be initialized")
            .clear_script_lists();
    }

    let mut outer_list = ScriptList::new();
    outer_list.append_script(Box::new(subroutine(
        "DifferentSideOuter",
        call_subroutine_action("DifferentSideInner"),
    )));

    let mut inner_list = ScriptList::new();
    inner_list.append_script(Box::new(subroutine(
        "DifferentSideInner",
        set_flag_action("different_side_subroutine_ran"),
    )));

    let result = with_script_engine_mut(|engine| {
        engine
            .set_script_list_for_player(0, Some(Box::new(outer_list)))
            .unwrap();
        engine
            .set_script_list_for_player(1, Some(Box::new(inner_list)))
            .unwrap();
        let found = engine.execute_subroutine_by_name("DifferentSideOuter")?;
        Ok::<_, GameLogicError>((
            found,
            engine
                .get_flag("different_side_subroutine_ran")
                .map(|flag| flag.value),
        ))
    });

    assert!(matches!(result, Some(Ok((true, Some(true))))));
}

#[test]
fn detached_script_is_reinserted_in_original_order_after_unwind() {
    let engine = ScriptEngine::new().unwrap();
    let mut list = ScriptList::new();

    let mut first = Script::new();
    first.script_name = "UnwindFirst".to_string();
    let mut second = Script::new();
    second.script_name = "UnwindSecond".to_string();
    list.append_script(Box::new(first));
    list.append_script(Box::new(second));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.with_active(|| {
            engine.with_active_script_lists(|| {
                engine
                    .with_detached_script(
                        ScriptLocation {
                            side_index: 0,
                            container: ScriptContainer::Root,
                            script_index: 0,
                        },
                        |_script| -> GameLogicResult<()> {
                            panic!("force Script execution stack unwind")
                        },
                    )
                    .unwrap();
            })
        });
    }));

    assert!(unwind.is_err());
    assert!(
        !ACTIVE_SCRIPT_LISTS.is_set(),
        "the active list scope must be cleared after unwinding"
    );
    let restored_names = engine.with_inner(|inner| {
        let first = inner.side_script_lists[0]
            .as_deref()
            .and_then(|list| list.first_script.as_deref())
            .expect("first Script must be restored");
        let second = first
            .next_script
            .as_deref()
            .expect("second Script must remain after restored first Script");
        (
            first.script_name.clone(),
            second.script_name.clone(),
            second.next_script.is_none(),
        )
    });
    assert_eq!(
        restored_names,
        ("UnwindFirst".to_string(), "UnwindSecond".to_string(), true)
    );
}

#[test]
fn same_script_reentry_is_explicit_and_restores_the_live_node() {
    let _lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();

    {
        let global = get_script_engine();
        let mut engine = global.write().unwrap();
        engine
            .as_mut()
            .expect("global ScriptEngine must be initialized")
            .clear_script_lists();
    }

    let mut list = ScriptList::new();
    list.append_script(Box::new(subroutine(
        "SafeRecursiveSubroutine",
        call_subroutine_action("SafeRecursiveSubroutine"),
    )));

    let result = with_script_engine_mut(|engine| {
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine.execute_subroutine_by_name("SafeRecursiveSubroutine")
    });

    assert!(matches!(
        result,
        Some(Err(GameLogicError::Configuration(message)))
            if message.contains("recursive CALL_SUBROUTINE")
    ));

    let global = get_script_engine();
    let engine = global.read().unwrap();
    let stored = engine
        .as_ref()
        .expect("global ScriptEngine must be initialized")
        .find_script_clone_by_name("SafeRecursiveSubroutine")
        .expect("the recursive Script node must be restored after the error");
    assert!(stored.is_active);
    assert!(stored.is_subroutine);
}

#[test]
fn with_active_script_engine_ref_skips_while_inner_mut_live() {
    let engine = ScriptEngine::new().unwrap();
    engine.with_active(|| {
        {
            let _guard = engine.lock_inner_mut();
            assert!(
                with_active_script_engine_ref(|e| e.with_inner(|i| i.num_counters)).is_none(),
                "shared TLS ref must skip while exclusive inner borrow is live"
            );
        }

        assert_eq!(
            with_active_script_engine_ref(|e| e.with_inner(|i| i.num_counters)),
            Some(1)
        );
    });
}

#[test]
fn lexical_active_engine_restores_outer_binding_after_nested_scope() {
    let outer = ScriptEngine::new().unwrap();
    let inner = ScriptEngine::new().unwrap();

    outer.with_active(|| {
        outer.set_counter("outer_before", 1).unwrap();
        assert_eq!(
            with_active_script_engine_ref(|engine| {
                engine
                    .get_counter("outer_before")
                    .map(|counter| counter.value)
            }),
            Some(Some(1))
        );

        inner.with_active(|| {
            inner.set_counter("inner", 2).unwrap();
            assert_eq!(
                with_active_script_engine_ref(|engine| {
                    engine.get_counter("inner").map(|counter| counter.value)
                }),
                Some(Some(2)),
                "nested lexical scope must expose the nested engine"
            );
            assert_eq!(
                with_active_script_engine_ref(|engine| {
                    engine
                        .get_counter("outer_before")
                        .map(|counter| counter.value)
                }),
                Some(None),
                "the nested scope must not accidentally use the outer engine"
            );
        });

        assert_eq!(
            with_active_script_engine_ref(|engine| {
                engine
                    .get_counter("outer_before")
                    .map(|counter| counter.value)
            }),
            Some(Some(1)),
            "leaving a nested scope restores the outer engine"
        );
    });
}

#[test]
fn lexical_active_engine_is_cleared_after_unwind() {
    let engine = ScriptEngine::new().unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.with_active(|| {
            assert!(ACTIVE_SCRIPT_ENGINE.is_set());
            panic!("test scoped TLS cleanup");
        });
    }));

    assert!(unwind.is_err());
    assert!(
        !ACTIVE_SCRIPT_ENGINE.is_set(),
        "scoped TLS must not retain a dead ScriptEngine reference after unwinding"
    );
    assert!(with_active_script_engine_mut(|_| ()).is_none());
}

#[test]
fn breeze_and_track_getters_return_owned_snapshots() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_current_track_name("intro".into());
    engine.set_breeze_info(0.5, 0.1, 0.0, 30, 0.0);

    let breeze = engine.get_breeze_info();
    let track = engine.get_current_track_name();
    assert_eq!(track, "intro");
    assert!((breeze.intensity - 0.1).abs() < f32::EPSILON);

    engine.set_current_track_name("combat".into());
    engine.set_breeze_info(1.0, 0.9, 0.0, 30, 0.0);

    assert_eq!(track, "intro", "owned snapshot must not alias inner");
    assert!(
        (breeze.intensity - 0.1).abs() < f32::EPSILON,
        "owned breeze snapshot must not alias inner"
    );
    assert_eq!(engine.get_current_track_name(), "combat");
    assert!((engine.get_breeze_info().intensity - 0.9).abs() < f32::EPSILON);
}
