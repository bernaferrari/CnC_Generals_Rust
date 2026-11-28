use super::*;
use crate::scripting::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator, ScriptConditionResult,
    ScriptContext,
};

#[test]
fn presentation_callbacks_follow_the_receiving_interpreter_across_map_and_reset() {
    #[derive(Debug, PartialEq, Eq)]
    enum Callback {
        Open(String),
        Close,
        CampaignVictory(bool),
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Callback>>);

    impl ScriptActionHandler for Recorder {
        fn create_win_lose_window(&self, layout: &str) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(Callback::Open(layout.into()));
            Ok(())
        }

        fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(Callback::Close);
            Ok(())
        }

        fn set_campaign_victorious(&self, victorious: bool) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(Callback::CampaignVictory(victorious));
            Ok(())
        }
    }

    let first_world = crate::system::engine_stores::new_world_services();
    let second_world = crate::system::engine_stores::new_world_services();
    let mut first = ScriptEngine::new_with_services(&first_world).unwrap();
    let mut second = ScriptEngine::new_with_services(&second_world).unwrap();
    let first_recorder = Arc::new(Recorder::default());
    let second_recorder = Arc::new(Recorder::default());
    first.set_action_handler(Some(first_recorder.clone()));
    second.set_action_handler(Some(second_recorder.clone()));

    first.with_inner(|_| {
        first.create_win_lose_window("Menus/Victorious.wnd");
        first.set_campaign_victorious(true);
    });
    second.create_win_lose_window("Menus/Defeat.wnd");
    first.new_map();
    assert_eq!(first.current_win_lose_window().as_deref(), Some("Menus/Victorious.wnd"));
    assert!(first.is_campaign_victorious());
    assert!(!second.is_campaign_victorious());

    first.close_windows(true);
    first.create_win_lose_window("suppressed.wnd");
    assert!(first.is_suppress_new_windows());
    assert!(first.current_win_lose_window().is_none());
    first.reset();
    assert!(!first.is_suppress_new_windows());
    assert!(!first.is_campaign_victorious());
    first.create_win_lose_window("after-reset.wnd");
    assert_eq!(second.current_win_lose_window().as_deref(), Some("Menus/Defeat.wnd"));

    assert_eq!(
        &*first_recorder.0.lock().unwrap(),
        &[
            Callback::Open("Menus/Victorious.wnd".into()),
            Callback::CampaignVictory(true),
            Callback::Close,
            Callback::Open("after-reset.wnd".into()),
        ],
    );
    assert_eq!(
        &*second_recorder.0.lock().unwrap(),
        &[Callback::Open("Menus/Defeat.wnd".into())],
    );
}

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
    let engine = ScriptEngine::new_in_published_world().unwrap();
    // Slot 0 is reserved, matching runtime reset semantics.
    assert_eq!(engine.allocate_counter("").unwrap(), 0);
    assert!(engine.snapshot_named_trackers().0.is_empty());
    assert_eq!(engine.allocate_flag("").unwrap(), 0);
    assert!(engine.snapshot_named_trackers().1.is_empty());
    assert_eq!(engine.runtime.frame().fade.mode, TFade::None);
    assert!(!engine.is_game_ending());
}

#[test]
fn receiving_xfer_tail_none_fade_starts_multiply_intro() {
    // hq-r9gaw: archive restore applies C++ FADE_NONE → Multiply intro.
    use crate::system::engine_stores::{new_world_services, with_world_services};
    let services = new_world_services();
    with_world_services(&services, || {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let mut tail = ScriptEngineXferTail::default();
        tail.fade = TFade::None as i32;
        engine.restore_receiving_xfer_tail(&tail);
        assert_eq!(engine.get_fade(), TFade::Multiply);
        assert_eq!(engine.get_fade_value(), 0.0);
        assert_eq!(engine.runtime.frame().fade.min, 1.0);
        assert_eq!(engine.runtime.frame().fade.max, 0.0);
        assert_eq!(engine.runtime.frame().fade.decrease, 33);
        assert_eq!(engine.runtime.frame().fade.frame, 0);
        // Active non-None fade must stay unchanged.
        let mut active = ScriptEngineXferTail::default();
        active.fade = TFade::Add as i32;
        active.cur_fade_value = 0.5;
        active.fade_frames_decrease = 10;
        engine.restore_receiving_xfer_tail(&active);
        assert_eq!(engine.get_fade(), TFade::Add);
        assert_eq!(engine.get_fade_value(), 0.5);
        assert_eq!(engine.runtime.frame().fade.decrease, 10);
    });
}

#[test]
fn test_new_map_starts_fade_in_from_black() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.reset();
    assert_eq!(engine.runtime.frame().fade.mode, TFade::None);
    engine.new_map();
    assert_eq!(engine.get_fade(), TFade::Multiply);
    assert_eq!(engine.get_fade_value(), 0.0);
    assert_eq!(engine.runtime.frame().fade.min, 1.0);
    assert_eq!(engine.runtime.frame().fade.max, 0.0);
    assert_eq!(engine.runtime.frame().fade.increase, 0);
    assert_eq!(engine.runtime.frame().fade.hold, 0);
    assert_eq!(engine.runtime.frame().fade.decrease, 33);
    assert_eq!(engine.runtime.frame().fade.frame, 0);
}

#[test]
fn test_counter_operations() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

    // Set a counter
    engine.set_counter("test_counter", 42).unwrap();
    assert_eq!(engine.allocate_counter("test_counter").unwrap(), 1);

    // Get the counter
    let counter = engine.get_counter("test_counter").unwrap();
    assert_eq!(counter.value, 42);
    assert_eq!(counter.name, "test_counter");
}

#[test]
fn test_flag_operations() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

    // Set a flag
    engine.set_flag("test_flag", true).unwrap();
    assert_eq!(engine.allocate_flag("test_flag").unwrap(), 1);

    // Get the flag
    let flag = engine.get_flag("test_flag").unwrap();
    assert!(flag.value);
    assert_eq!(flag.name, "test_flag");
}

#[test]
fn test_end_game_timer() {
    let engine = ScriptEngine::new_in_published_world().unwrap();
    assert!(!engine.is_game_ending());

    engine.start_end_game_timer();
    assert!(engine.is_game_ending());
    assert_eq!(engine.runtime.frame().mission_timers.end_game, 120);

    engine.start_quick_end_game_timer();
    assert_eq!(engine.runtime.frame().mission_timers.end_game, 1);

    engine.start_close_window_timer();
    assert_eq!(engine.runtime.frame().mission_timers.close_window, 120);
}

#[test]
fn end_game_timer_expiry_appends_clear_game_data_message() {
    // C++ ScriptEngine.cpp:5514-5518 TheMessageStream->appendMessage(MSG_CLEAR_GAME_DATA)
    use game_engine::common::message_stream::{GameMessageType, get_message_stream};

    let _lock = crate::test_sync::lock();
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    {
        let stream = get_message_stream();
        let mut stream = stream.write().expect("message stream");
        stream.clear_messages();
    }
    engine.start_quick_end_game_timer();
    engine
        .update()
        .expect("update should expire the 1-frame timer");
    let stream = get_message_stream();
    let stream = stream.read().expect("message stream");
    assert!(
        stream.contains_message_of_type(&GameMessageType::ClearGameData),
        "end-game timer expiry must append MSG_CLEAR_GAME_DATA, not clearGameData() directly"
    );
}

#[test]
fn close_window_timer_expiry_destroys_message_window() {
    // C++ ScriptEngine.cpp:5508-5512 TheScriptActions->closeWindows(FALSE)
    let _lock = crate::test_sync::lock();
    struct HeadlessWindows;
    impl ScriptActionHandler for HeadlessWindows {}
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_action_handler(Some(std::sync::Arc::new(HeadlessWindows)));
    engine.create_win_lose_window("Menus/LocalDefeat.wnd");
    assert_eq!(
        engine.current_win_lose_window().as_deref(),
        Some("Menus/LocalDefeat.wnd")
    );
    engine.start_close_window_timer();
    // C++ decrements while >0 and closes when the remaining count drops below 1.
    for _ in 0..120 {
        engine.update().expect("close-window countdown");
    }
    assert_eq!(
        engine.current_win_lose_window(),
        None,
        "close-window timer expiry must destroy the win/lose message window"
    );
}

#[test]
fn test_time_freeze() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    assert!(!engine.is_time_frozen_script());

    engine.do_freeze_time();
    assert!(engine.is_time_frozen_script());

    engine.do_unfreeze_time();
    assert!(!engine.is_time_frozen_script());
}

#[test]
fn test_debug_freeze_stops_update_progression() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
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
fn countdown_timer_rests_at_minus_one_after_expire() {
    // C++ ScriptEngine.cpp:5547-5550: decrement while value >= 0, rest at -1.
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_timer("expire_to_minus_one", 0).unwrap();
    engine.update().expect("update from 0");
    assert_eq!(
        engine.get_counter("expire_to_minus_one").unwrap().value,
        -1,
        "expired countdown must rest at -1, not 0"
    );
    engine.update().expect("update while already -1");
    assert_eq!(
        engine.get_counter("expire_to_minus_one").unwrap().value,
        -1,
        "countdown must stop at -1"
    );
}

#[test]
fn named_trackers_resume_in_the_receiving_interpreter_with_both_codecs() {
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    struct Headless;

    impl ScriptExecutionDriver<ScriptEngine> for Headless {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }

        fn initialize_named_cache(&mut self) -> Option<()> {
            Some(())
        }
    }

    let donor_world = new_world_services();
    let mut donor = ScriptEngine::new_with_services(&donor_world).unwrap();
    donor.new_map();
    donor.set_timer("Deadline", 2).unwrap();
    donor.set_counter("Score", -3).unwrap();
    donor.set_flag("Ready0", true).unwrap();
    donor.start_close_window_timer();
    donor.set_topple_direction("FirstStructure", Some(glam::Vec3::X));
    donor.set_topple_direction("SecondStructure", Some(glam::Vec3::new(0.0, 7.0, 0.0)));
    donor.set_topple_direction("FirstStructure", Some(glam::Vec3::new(3.0, 4.0, 0.0)));
    donor
        .update_with_driver(ScriptContext::at_frame(1), &mut Headless)
        .unwrap();
    let saved = donor.snapshot_named_trackers();
    let tail = with_world_services(&donor_world, || donor.snapshot_xfer_tail());
    assert_eq!(
        tail.topple_directions,
        [
            ("SecondStructure".into(), [0.0, 7.0, 0.0]),
            ("FirstStructure".into(), [3.0, 4.0, 0.0]),
        ]
    );

    let mut wire = Cursor::new(Vec::new());
    with_world_services(&donor_world, || {
        donor.xfer(&mut XferSave::new(&mut wire, 1)).unwrap();
    });
    let wire = wire.into_inner();

    let legacy_world = new_world_services();
    let mut legacy = ScriptEngine::new_with_services(&legacy_world).unwrap();
    with_world_services(&legacy_world, || {
        legacy
            .xfer(&mut XferLoad::new(Cursor::new(&wire), 1))
            .unwrap();
        let mut resaved = Cursor::new(Vec::new());
        legacy.xfer(&mut XferSave::new(&mut resaved, 1)).unwrap();
        assert_eq!(resaved.into_inner(), wire);
    });

    let receiving_world = new_world_services();
    let receiving = ScriptEngine::new_with_services(&receiving_world).unwrap();
    receiving.restore_named_trackers(&saved.0, &saved.1).unwrap();
    receiving.restore_receiving_xfer_tail(&tail);
    assert_eq!(receiving.snapshot_named_trackers(), saved);
    assert_eq!(legacy.snapshot_named_trackers(), saved);
    for engine in [&legacy, &receiving] {
        assert_eq!(
            engine.topple_direction_for_name("FirstStructure"),
            Some(glam::Vec3::new(3.0, 4.0, 0.0).normalize())
        );
        assert_eq!(
            engine.topple_direction_for_name("SecondStructure"),
            Some(glam::Vec3::Y)
        );
    }
    donor.set_topple_direction("SecondStructure", None);
    assert_eq!(receiving.topple_direction_for_name("SecondStructure"), Some(glam::Vec3::Y));
    assert_eq!(legacy.topple_direction_for_name("SecondStructure"), Some(glam::Vec3::Y));

    donor.clear_flag("Ready");
    for (frame, remaining) in [(2, 0), (3, -1), (4, -1)] {
        for (world, engine) in [(&legacy_world, &legacy), (&receiving_world, &receiving)] {
            engine
                .update_with_driver(ScriptContext::at_frame(frame), &mut Headless)
                .unwrap();
            assert_eq!(engine.get_counter("Deadline").unwrap().value, remaining);
            assert_eq!(engine.get_counter("Score").unwrap().value, -3);
            assert!(engine.get_flag("Ready0").unwrap().value);
            let close_window = with_world_services(world, || engine.snapshot_xfer_tail())
                .close_window_timer;
            assert_eq!(close_window, 120 - frame as i32);
        }
    }
    assert_eq!(donor.get_counter("Deadline").unwrap().value, 1);
    assert!(!donor.get_flag("Ready0").unwrap().value);
    assert_eq!(
        with_world_services(&donor_world, || donor.snapshot_xfer_tail()).close_window_timer,
        119
    );
}

#[test]
fn qualify_my_inner_perimeter_rewrites_to_player_start_index() {
    // C++ ScriptEngine.cpp:5890-5897 MyInnerPerimeter -> InnerPerimeter{mpStart+1}
    use crate::player::{Player, player_list};
    use std::sync::{Arc, RwLock};

    let _lock = crate::test_sync::lock();
    let mut player = Player::new(0);
    player.set_display_name("QualifyPerimeterPlayer");
    player.set_mp_start_index(2);
    {
        let __player_list_arc = player_list();
        let mut list = __player_list_arc.write().expect("player list");
        list.clear();
        list.add_player(Arc::new(RwLock::new(player)));
    }

    assert_eq!(
        qualify_trigger_area_name(MY_INNER_PERIMETER, Some("QualifyPerimeterPlayer")).as_deref(),
        Some("InnerPerimeter3")
    );
    assert_eq!(
        qualify_trigger_area_name(MY_OUTER_PERIMETER, Some("QualifyPerimeterPlayer")).as_deref(),
        Some("OuterPerimeter3")
    );
    assert_eq!(
        qualify_trigger_area_name(MY_INNER_PERIMETER, None),
        None,
        "C++ returns NULL when m_currentPlayer is missing"
    );

    player_list().write().expect("player list").clear();
}

#[test]
fn sequential_pending_wait_conversion_matches_cxx_wait_patterns() {
    // Retry-style wait actions re-run the same instruction on the next frame.
    assert_eq!(
        generals_sim::scripting::sequential_scripts::pending_wait_frames(1.0, true),
        0
    );
    assert_eq!(
        generals_sim::scripting::sequential_scripts::pending_wait_frames(3.0, true),
        2
    );

    // Framecount actions wait before advancing to the next instruction.
    assert_eq!(
        generals_sim::scripting::sequential_scripts::pending_wait_frames(1.0, false),
        1
    );
    assert_eq!(
        generals_sim::scripting::sequential_scripts::pending_wait_frames(3.0, false),
        3
    );
}

#[test]
fn sequential_pending_retry_action_classification_matches_cxx() {
    assert!(
        generals_sim::scripting::sequential_scripts::repeats_instruction(
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
        )
    );
    assert!(
        generals_sim::scripting::sequential_scripts::repeats_instruction(
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
        )
    );
    assert!(
        generals_sim::scripting::sequential_scripts::repeats_instruction(
            ScriptActionType::TeamWaitForNotContainedAll
        )
    );
    assert!(
        generals_sim::scripting::sequential_scripts::repeats_instruction(
            ScriptActionType::TeamWaitForNotContainedPartial
        )
    );
    assert!(
        !generals_sim::scripting::sequential_scripts::repeats_instruction(
            ScriptActionType::TeamGuardForFramecount
        )
    );

    assert!(is_sequential_only_action(
        ScriptActionType::TeamGuardForFramecount
    ));
    assert!(is_sequential_only_action(
        ScriptActionType::TeamWaitForNotContainedAll
    ));
    assert!(!is_sequential_only_action(
        ScriptActionType::TeamGuardPosition
    ));
}

#[test]
fn set_script_list_initializes_delay_evaluation_frame_offset() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

    let mut delayed_script = Script::new();
    delayed_script.delay_evaluation_seconds = 3;
    delayed_script.frame_to_evaluate_at = 9999;

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(delayed_script));

    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    let frame = engine
        .find_script_clone_by_name("")
        .expect("script should be present")
        .frame_to_evaluate_at;
    assert!(frame <= (2 * LOGICFRAMES_PER_SECOND as u32));
}

#[test]
fn set_script_list_infers_condition_team_name_for_singleton_team() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
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
        .find_script_clone_by_name("")
        .expect("script should be present")
        .condition_team_name;
    assert_eq!(condition_team, team_name);
}

#[test]
fn call_subroutine_executes_in_place_and_persists_one_shot_state() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

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

    assert!(
        engine
            .execute_subroutine_by_name("SubroutinePersist")
            .unwrap()
    );

    let is_active = engine
        .find_script_clone_by_name("SubroutinePersist")
        .expect("subroutine should still exist")
        .is_active;
    assert!(!is_active);
}

#[test]
fn call_subroutine_resolves_subroutine_group_name_first() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

    let mut condition = Condition::new(ConditionType::ConditionTrue);
    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(condition)));

    let mut grouped_subroutine = Script::new();
    grouped_subroutine.script_name = "InnerGroupedSubroutine".to_string();
    grouped_subroutine.is_subroutine = false;
    grouped_subroutine.is_one_shot = true;
    grouped_subroutine.condition = Some(Box::new(or_condition));

    let mut group = crate::scripting::core::new_script_group();
    group.group_name = "NamedSubroutineGroup".to_string();
    group.is_group_subroutine = true;
    group.is_group_active = true;
    group.first_script = Some(Box::new(grouped_subroutine));

    let mut script_list = ScriptList::new();
    script_list.first_group = Some(Box::new(group));
    engine
        .set_script_list_for_player(0, Some(Box::new(script_list)))
        .unwrap();

    assert!(
        engine
            .execute_subroutine_by_name("NamedSubroutineGroup")
            .unwrap()
    );

    let grouped_active = engine
        .find_script_clone_by_name("InnerGroupedSubroutine")
        .expect("grouped subroutine should exist")
        .is_active;
    assert!(!grouped_active);
}

#[test]
fn enable_disable_script_by_group_name_toggles_cxx_group_state() {
    // C++ ScriptEngine::enableScript/disableScript (ScriptEngine.cpp:6797-6823)
    // first-classifies a matching ScriptGroup separately from matching Script.
    // An inactive subroutine group must become callable immediately when enabled
    // and must stop being callable when disabled.
    let mut engine = ScriptEngine::new_in_published_world().unwrap();

    let mut member = Script::new();
    member.script_name = "Toggle Group Member".to_string();
    member.condition = Some(always_true_condition());
    member.action = Some(set_flag_action("group_toggle_fired"));

    let mut group = crate::scripting::core::new_script_group();
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
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine
        .set_timer_millisecond_script_seconds("shell_camera", 0.12)
        .unwrap();

    let counter = engine
        .get_counter("shell_camera")
        .expect("timer counter");
    assert_eq!(counter.value, 4);
    assert!(counter.is_countdown_timer);
}

#[test]
fn stop_timer_preserves_remaining_value() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_timer("pause_test", 17).unwrap();
    engine.stop_timer("pause_test").unwrap();

    let counter = engine
        .get_counter("pause_test")
        .expect("pause counter");
    assert_eq!(counter.value, 17);
    assert!(!counter.is_countdown_timer);
}

#[test]
fn subtract_millisecond_script_seconds_can_drive_timer_negative() {
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    engine.set_timer("negative_test", 2).unwrap();
    engine
        .subtract_from_timer_millisecond_script_seconds("negative_test", 0.12)
        .unwrap();

    let counter = engine
        .get_counter("negative_test")
        .expect("negative counter");
    assert_eq!(counter.value, -1);
}

/// C++ `ScriptEngine::executeActions` handles `CALL_SUBROUTINE` by calling
/// `callSubroutine` → `executeScript` **immediately**, then continues the
/// remaining outer actions. Nested `with_script_engine_mut` must not panic
/// and must observe outer mutations already applied.
#[test]
fn nested_with_script_engine_mut_runs_immediately_like_cxx_call_subroutine() {
    let engine = ScriptEngine::new_in_published_world().unwrap();
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
        let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut dispatcher = ScriptActionDispatcher::new(&engine, &dispatcher_state);

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
            engine.runtime.named_reveal(reveal_name).is_some(),
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
        assert!(engine.runtime.named_reveal(reveal_name).is_none());
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
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
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
        .with_active(|| engine.evaluate_sequential_scripts_canonical_for_test())
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
    let engine = ScriptEngine::new_in_published_world().unwrap();
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
        engine.runtime.sides().execute_side(0, |_script| {
            panic!("force Script execution stack unwind")
        }).unwrap();
    }));

    assert!(unwind.is_err());
    engine
        .capture_side_script_continuation()
        .expect("save boundary restored after unwinding");
    let first = engine
        .find_script_clone_by_name("UnwindFirst")
        .expect("first Script restored");
    let second = first
        .get_next()
        .expect("second Script remains after the restored first Script");
    let restored_names = (
        first.script_name.clone(),
        second.script_name.clone(),
        second.get_next().is_none(),
    );
    assert_eq!(
        restored_names,
        ("UnwindFirst".to_string(), "UnwindSecond".to_string(), true)
    );
}

#[test]
fn nested_subroutine_uses_its_receiving_worlds_side_lists() {
    use crate::system::engine_stores::new_world_services;

    struct PeerAction<'a> {
        engine: &'a ScriptEngine,
        ran: bool,
    }
    impl ScriptExecutionDriver<ScriptEngine> for PeerAction<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            assert!(self.engine.capture_side_script_continuation().is_err());
            self.ran = true;
            Ok(())
        }
    }

    struct CallPeer<'a> {
        engine: &'a ScriptEngine,
        peer: PeerAction<'a>,
    }
    impl ScriptExecutionDriver<ScriptEngine> for CallPeer<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            assert!(self.engine.capture_side_script_continuation().is_err());
            assert!(self.peer.engine.capture_side_script_continuation().is_ok());
            assert!(self.peer.engine.execute_subroutine_by_name_with_driver(
                "PeerOnly",
                ScriptContext::at_frame(7),
                &mut self.peer,
            )?);
            assert!(self.engine.capture_side_script_continuation().is_err());
            assert!(self.peer.engine.capture_side_script_continuation().is_ok());
            Ok(())
        }
    }

    let outer_world = new_world_services();
    let peer_world = new_world_services();
    let outer = ScriptEngine::new_with_services(&outer_world).unwrap();
    let peer = ScriptEngine::new_with_services(&peer_world).unwrap();
    for (engine, name) in [(&outer, "OuterOnly"), (&peer, "PeerOnly")] {
        let mut condition = OrCondition::new();
        condition.first_and = Some(Box::new(Condition::new(ConditionType::ConditionTrue)));
        let mut script = Script::new();
        script.script_name = name.into();
        script.is_subroutine = true;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(ScriptAction::new(ScriptActionType::NoOp)));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        engine.set_script_list_for_player(0, Some(Box::new(list))).unwrap();
    }

    let mut driver = CallPeer {
        engine: &outer,
        peer: PeerAction { engine: &peer, ran: false },
    };
    assert!(outer.execute_subroutine_by_name_with_driver(
        "OuterOnly",
        ScriptContext::at_frame(7),
        &mut driver,
    ).unwrap());
    assert!(driver.peer.ran);
    assert!(!outer.find_script_clone_by_name("OuterOnly").unwrap().is_active);
    assert!(!peer.find_script_clone_by_name("PeerOnly").unwrap().is_active);
    assert!(outer.capture_side_script_continuation().is_ok());
    assert!(peer.capture_side_script_continuation().is_ok());
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
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine.with_active(|| {
        {
            let _guard = engine.lock_inner_mut();
            assert!(
                with_active_script_engine_ref(|e| e.get_counter("").is_some()).is_none(),
                "shared TLS ref must skip while exclusive inner borrow is live"
            );
        }
        {
            let _frame = engine.runtime.frame_mut();
            assert!(with_active_script_engine_ref(|e| e.get_counter("")).is_none());
        }
        {
            let _priorities = engine.runtime.attack_priorities_mut();
            assert!(with_active_script_engine_ref(|e| e.get_attack_info("")).is_none());
        }

        assert_eq!(
            with_active_script_engine_ref(|e| e.get_counter("").is_some()),
            Some(true)
        );
    });
}

#[test]
fn lexical_active_engine_restores_outer_binding_after_nested_scope() {
    let outer = ScriptEngine::new_in_published_world().unwrap();
    let inner = ScriptEngine::new_in_published_world().unwrap();

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
    let engine = ScriptEngine::new_in_published_world().unwrap();
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
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
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

#[test]
fn script_engine_xfer_tail_round_trips_freeze_timers_and_named_units() {
    let _lock = crate::test_sync::lock();
    get_named_object_tracker().clear().unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine.do_freeze_time();
    engine.start_end_game_timer();
    engine.set_current_track_name("Combat".into());
    engine.set_choose_victim_always_uses_normal(true);
    engine.set_objects_should_receive_difficulty_bonus(false);
    engine.set_object_attack_priority_set(42, "Heroes");
    get_named_object_tracker()
        .register_named_object("NamedHero".to_string(), 42)
        .unwrap();

    let tail = engine.snapshot_xfer_tail();
    assert!(tail.freeze_by_script);
    assert_eq!(tail.end_game_timer, 120);
    assert_eq!(tail.current_track_name, "Combat");
    assert!(tail.choose_victim_always_uses_normal);
    assert!(!tail.objects_should_receive_difficulty_bonus);
    assert_eq!(tail.object_attack_priority_sets[0], (42, "Heroes".into()));
    assert!(
        tail.named_objects
            .iter()
            .any(|(name, id)| name == "NamedHero" && *id == 42)
    );

    let restored = ScriptEngine::new_in_published_world().unwrap();
    restored.restore_xfer_tail(&tail);
    assert!(restored.is_time_frozen_script());
    assert!(restored.is_game_ending());
    assert_eq!(restored.get_current_track_name(), "Combat");
    assert!(restored.get_choose_victim_always_uses_normal());
    assert!(!restored.get_objects_should_receive_difficulty_bonus());
    assert_eq!(
        restored.get_object_attack_priority_set(42).as_deref(),
        Some("Heroes")
    );
    assert_eq!(
        get_named_object_tracker()
            .get_object_id("NamedHero")
            .unwrap(),
        Some(42)
    );
}

#[test]
fn empty_object_registry_still_runs_true_side_scripts() {
    // C++ ScriptEngine.cpp:5554-5571 walks every side even with an empty
    // object table. TRUE/FLAG scripts must still fire on the live host
    // (OBJECT_REGISTRY is always empty there).
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut script = Script::new();
    script.script_name = "EmptyRegistryTrue".to_string();
    script.is_active = true;
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = Some(set_flag_action("empty_registry_true_ran"));

    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    engine
        .update()
        .expect("side scripts must run with an empty OBJECT_REGISTRY");
    assert!(
        engine
            .get_flag("empty_registry_true_ran")
            .is_some_and(|flag| flag.value),
        "TRUE SET_FLAG must fire when leftover OBJECT_REGISTRY is empty"
    );
}

#[test]
fn empty_object_registry_still_progresses_sequential_scripts() {
    // C++ ScriptEngine.cpp:7860 evaluateAndProgressAllSequentialScripts does
    // not abort on an empty object table. A node with no object/team is
    // cleaned up instead of being left stuck.
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut sequence = SequentialScript::new();
    sequence.script_to_execute_sequentially = Some(Box::new({
        let mut script = Script::new();
        script.script_name = "OrphanSequential".to_string();
        script.action = Some(set_flag_action("orphan_sequential"));
        script
    }));
    engine.append_sequential_script(sequence);
    assert!(engine.sequential_script_count() > 0);

    engine
        .with_active(|| engine.evaluate_sequential_scripts_canonical_for_test())
        .expect("sequential walk must not fail-closed on empty OBJECT_REGISTRY");
    assert_eq!(
        engine.sequential_script_count(),
        0,
        "orphan sequential node must be cleaned up when no object/team exists"
    );
}

#[test]
fn leftover_team_sequential_progresses_when_host_members_idle() {
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
    get_team_factory().lock().unwrap().reset();
    {
        let published_team_factory = get_team_factory();
        let mut factory = published_team_factory.lock().unwrap();
        factory.init_team(
            crate::common::AsciiString::from("HostSeqTeam"),
            crate::common::AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("HostSeqTeam")
            .expect("leftover team should exist");
    }
    crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 42,
            name: "SeqRanger".into(),
            alive: true,
            idle: true,
            ..Default::default()
        }],
        team_instance_ids: [("HostSeqTeam".into(), vec![42])].into_iter().collect(),
        ..Default::default()
    });

    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut sequence = SequentialScript::new();
    sequence.team_to_exec_on = get_team_factory()
        .lock()
        .unwrap()
        .find_team_instances("HostSeqTeam")[0]
        .read()
        .ok()
        .map(|team| team.get_id());
    sequence.script_to_execute_sequentially = Some(Box::new({
        let mut script = Script::new();
        script.script_name = "HostTeamSeq".to_string();
        script.action = Some(set_flag_action("host_team_seq_ran"));
        script
    }));
    engine.append_sequential_script(sequence);
    engine
        .with_active(|| engine.evaluate_sequential_scripts_canonical_for_test())
        .expect("host team sequential must progress when leftover members are empty");
    assert!(
        engine
            .get_flag("host_team_seq_ran")
            .is_some_and(|flag| flag.value),
        "SET_FLAG in a host-team sequential script must run"
    );
    get_team_factory().lock().unwrap().reset();
    crate::scripting::HostScriptQuery::published_for_test().clear_host_script_query_snapshot();
}

#[test]
fn sequential_scripts_snapshot_preserves_count_and_instruction() {
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let mut sequence = SequentialScript::new();
    sequence.object_id = 0x51_5E_0002;
    sequence.current_instruction = 4;
    sequence.times_to_loop = 3;
    sequence.frames_to_wait = 12;
    sequence.dont_advance_instruction = true;
    sequence.script_to_execute_sequentially = Some(Box::new({
        let mut script = Script::new();
        script.script_name = "PersistedHunt".to_string();
        script
    }));
    engine.runtime.sequential().restore_heads([sequence]);

    let snapshot = engine.snapshot_sequential_scripts();
    assert_eq!(snapshot.len(), 1, "C++ xfer writes sequential count, not 0");
    assert_eq!(snapshot[0].object_id, 0x51_5E_0002);
    assert_eq!(snapshot[0].script_name, "PersistedHunt");
    assert_eq!(snapshot[0].current_instruction, 4);
    assert_eq!(snapshot[0].times_to_loop, 3);
    assert_eq!(snapshot[0].frames_to_wait, 12);
    assert!(snapshot[0].dont_advance_instruction);

    engine.restore_sequential_scripts(&snapshot);
    let restored = engine.snapshot_sequential_scripts();
    assert_eq!(restored, snapshot);
    assert_eq!(
        engine.sequential_script_count(),
        1,
        "restore must not drop sequential instances"
    );
}

#[test]
fn named_reveals_snapshot_survives_restore_for_undo() {
    let _lock = crate::test_sync::lock();
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine.create_named_map_reveal("BaseLook", "WP_Base", 250.0, "PlyrAmerica");
    engine.create_named_map_reveal("HillLook", "WP_Hill", 80.0, "PlyrChina");

    let snapshot = engine.snapshot_named_reveals();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].0, "BaseLook");
    assert_eq!(snapshot[0].1, "WP_Base");
    assert!((snapshot[0].2 - 250.0).abs() < f32::EPSILON);
    assert_eq!(snapshot[0].3, "PlyrAmerica");

    engine.restore_named_reveals(&[]);
    assert!(engine.snapshot_named_reveals().is_empty());

    engine.restore_named_reveals(&snapshot);
    assert_eq!(engine.snapshot_named_reveals(), snapshot);

    // C++ undoNamedMapReveal + removeNamedMapReveal after load needs the list.
    engine.undo_named_map_reveal("BaseLook");
    engine.remove_named_map_reveal("BaseLook");
    let remaining = engine.snapshot_named_reveals();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].0, "HillLook");
}

#[test]
fn eval_flag_true_when_ui_interaction_name_matches_like_cxx() {
    // C++ ScriptEngine::evaluateFlag (ScriptEngine.cpp:6450-6458): stored flag
    // miss is still true if m_uiInteractions contains the flag name.
    let _lock = crate::test_sync::lock();
    let engine = ScriptEngine::new_in_published_world().unwrap();

    const HOOK: &str = "ShellMainMenuOnlineHighlighted";
    {
        engine.set_flag(HOOK, false).unwrap();
        engine.signal_ui_interact(HOOK);
        assert!(engine.has_ui_interaction(HOOK));
        assert!(!engine.has_ui_interaction("OtherFlag"));
    }

    let mut pulse = Condition::new(ConditionType::Flag);
    pulse
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            HOOK.to_string(),
        ))
        .unwrap();
    pulse
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();

    let mut other = Condition::new(ConditionType::Flag);
    other
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "OtherFlag".to_string(),
        ))
        .unwrap();
    other
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();

    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &evaluator_state);
    assert_eq!(
        evaluator.evaluate_condition(&mut pulse).unwrap(),
        ScriptConditionResult::True,
        "C++ evaluateFlag returns true when the UI hook name matches"
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut other).unwrap(),
        ScriptConditionResult::False,
        "unrelated flag names stay false without a stored flag or pulse"
    );
}

fn flag_equals_true_condition(flag_name: &str) -> Box<OrCondition> {
    let mut flag = Condition::new(ConditionType::Flag);
    flag.add_parameter(Parameter::with_string(
        ParameterType::Flag,
        flag_name.to_string(),
    ))
    .unwrap();
    flag.add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    let mut or_condition = OrCondition::new();
    or_condition.set_first_and_condition(Some(Box::new(flag)));
    Box::new(or_condition)
}

#[test]
fn live_update_flag_true_when_ui_interaction_pulses_like_cxx() {
    // Live host (scripts_camera.rs) takes ScriptEngine out of the global
    // RwLock then calls update(). C++ evaluateFlag is true when
    // m_uiInteractions contains the hook name; pulses clear after
    // executeScripts (ScriptEngine.cpp:5577-5578).
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();

    const HOOK: &str = "ShellMainMenuOnlineHighlighted";
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    let mut script = Script::new();
    script.script_name = "UiPulseFlag".to_string();
    script.is_active = true;
    script.is_one_shot = true;
    script.condition = Some(flag_equals_true_condition(HOOK));
    script.action = Some(set_flag_action("ui_pulse_fired"));

    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();

    engine.set_flag(HOOK, false).unwrap();
    engine.signal_ui_interact(HOOK);
    assert!(engine.has_ui_interaction(HOOK));

    engine
        .update()
        .expect("live ScriptEngine::update must evaluate FLAG");
    assert!(
        engine
            .get_flag("ui_pulse_fired")
            .is_some_and(|flag| flag.value),
        "C++ evaluateFlag must fire the FLAG==TRUE script from a UI pulse"
    );
    assert!(
        !engine.has_ui_interaction(HOOK),
        "C++ clears m_uiInteractions after executeScripts"
    );
}

#[test]
fn live_host_take_engine_update_flag_ui_pulse_like_cxx() {
    // scripts_camera.rs: take engine out of the global lock, update(), put back.
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    initialize_script_engine().unwrap();

    const HOOK: &str = "ShellMainMenuSkirmishHighlighted";
    {
        let handle = get_script_engine();
        let mut guard = handle.write().unwrap();
        let engine = guard.as_mut().unwrap();
        engine.reset();
        let mut script = Script::new();
        script.script_name = "HostUiPulseFlag".to_string();
        script.is_active = true;
        script.is_one_shot = true;
        script.condition = Some(flag_equals_true_condition(HOOK));
        script.action = Some(set_flag_action("host_ui_pulse_fired"));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine.set_flag(HOOK, false).unwrap();
        engine.signal_ui_interact(HOOK);
    }

    crate::helpers::TheScriptEngine::signal_ui_interact(HOOK);

    let taken = match get_script_engine().write() {
        Ok(mut guard) => guard.take(),
        Err(_) => panic!("script engine lock poisoned"),
    };
    let engine = taken.expect("live host ScriptEngine must exist");
    engine
        .update()
        .expect("taken-out live update must evaluate FLAG");
    assert!(
        engine
            .get_flag("host_ui_pulse_fired")
            .is_some_and(|flag| flag.value),
        "live take-out-of-lock update must honor UI pulse like C++ evaluateFlag"
    );
    assert!(
        !engine.has_ui_interaction(HOOK),
        "taken-out update must clear the one-frame UI pulse"
    );
    if let Ok(mut guard) = get_script_engine().write() {
        *guard = Some(engine);
    }
}

mod borrowed_driver_tests;
mod display_driver_tests;

mod camera_driver_tests;

mod completion_driver_tests;
mod facade_owner_tests;
mod science_callback_owner_tests;

mod remaining_camera_driver_tests;

mod attack_priority_table_tests;

mod chunk_decode_owner_tests;

mod chunk_explicit_owner_tests;

mod chunk_writer_owner_tests;

#[path = "tests/authored_template_tests.rs"]
mod authored_template_tests;

#[path = "tests/authored_metadata_tests.rs"]
mod authored_metadata_tests;

mod timed_audio_parity_tests;

mod depth_owner_tests;

/// C++ ScriptActions::doNamedSetEvacLeftOrRight stores on the engine's own
/// world. Two worlds take turns on ONE thread: an action run by world A's
/// engine while world B is published lands in A's map, and vice versa.
#[test]
fn named_evac_disposition_lands_in_the_dispatching_engines_world() {
    let _lock = crate::test_sync::lock();
    use crate::system::engine_stores::{new_world_services, with_world_services};
    let world_a = new_world_services();
    let world_b = new_world_services();
    let engine_a = ScriptEngine::new_with_services(&world_a).expect("engine A");
    let engine_b = ScriptEngine::new_with_services(&world_b).expect("engine B");
    let set_evac = |engine: &ScriptEngine, side: i32| {
        let state = std::cell::RefCell::new(ScriptContext::at_frame(1));
        let mut dispatcher = ScriptActionDispatcher::new(engine, &state);
        let mut action = ScriptAction::new(ScriptActionType::NamedSetEvacLeftOrRight);
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, "EvacBunker".to_string()))
            .expect("unit parameter");
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, side))
            .expect("side parameter");
        dispatcher.execute_action(&action).expect("evac action")
    };
    with_world_services(&world_b, || set_evac(&engine_a, 1));
    with_world_services(&world_a, || set_evac(&engine_b, 2));
    let read = |world: &std::sync::Arc<crate::system::engine_stores::WorldServices>| {
        crate::object::contain::named_evac_disposition(world.named_evac_disposition(), "EvacBunker")
    };
    assert_eq!(read(&world_a), Some(1), "A's engine wrote A's map");
    assert_eq!(read(&world_b), Some(2), "B's engine wrote B's map");
}

/// C++ ScriptActions::doEvaEnabledDisabled toggles the dispatching engine's
/// TheEva. Two worlds take turns on ONE thread, each engine dispatching while
/// the peer world is published; only the dispatching world's Eva changes.
#[test]
fn eva_set_enabled_lands_in_the_dispatching_engines_world() {
    let _lock = crate::test_sync::lock();
    use crate::helpers::{EvaEvent, TheEva};
    use crate::system::engine_stores::{new_world_services, with_world_services};
    let world_a = new_world_services();
    let world_b = new_world_services();
    let engine_a = ScriptEngine::new_with_services(&world_a).expect("engine A");
    let engine_b = ScriptEngine::new_with_services(&world_b).expect("engine B");
    let set_eva = |engine: &ScriptEngine, enabled: bool| {
        let state = std::cell::RefCell::new(ScriptContext::at_frame(1));
        let mut dispatcher = ScriptActionDispatcher::new(engine, &state);
        let mut action = ScriptAction::new(ScriptActionType::EvaSetEnabledDisabled);
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, i32::from(enabled)))
            .expect("enabled parameter");
        dispatcher.execute_action(&action).expect("eva action")
    };
    TheEva::set_should_play_in(&world_a, EvaEvent::LowPower).unwrap();
    TheEva::set_should_play_in(&world_b, EvaEvent::LowPower).unwrap();
    // Turn 1: A disables while B is published.
    with_world_services(&world_b, || set_eva(&engine_a, false));
    assert!(!TheEva::is_enabled_in(&world_a).unwrap());
    assert!(TheEva::is_enabled_in(&world_b).unwrap());
    assert!(TheEva::drain_events_in(&world_a).unwrap().is_empty(), "disable clears A's queue");
    assert_eq!(TheEva::drain_events_in(&world_b).unwrap(), vec![EvaEvent::LowPower]);
    // Turn 2: B disables while A is published; turn 3: A re-enables under B.
    with_world_services(&world_a, || set_eva(&engine_b, false));
    with_world_services(&world_b, || set_eva(&engine_a, true));
    assert!(TheEva::is_enabled_in(&world_a).unwrap());
    assert!(!TheEva::is_enabled_in(&world_b).unwrap());
}

/// C++ each engine's script actions read their own TheControlBar. Two worlds
/// take turns on ONE thread: each engine sees exactly its world's resolved
/// catalog, including a refresh made while the peer world is published.
#[test]
fn script_engine_control_bar_follows_its_own_world_slot() {
    let _lock = crate::test_sync::lock();
    use crate::system::engine_stores::{new_world_services, with_world_services};
    const NAME: &str = "Command_EngineOwnedBridgeButton";
    let world_a = new_world_services();
    let world_b = new_world_services();
    world_a.common().ini.control_bar.write().new_command_button(NAME.into()).command = "STOP".into();
    let engine_a = ScriptEngine::new_with_services(&world_a).expect("engine A");
    let engine_b = ScriptEngine::new_with_services(&world_b).expect("engine B");
    assert!(engine_a.control_bar_bridge().is_none());
    // A's world rebuilds its catalog while B is published on this thread.
    with_world_services(&world_b, || {
        crate::control_bar::refresh_control_bar_bridge_for_world(&world_a, None).unwrap();
    });
    for published in [&world_a, &world_b, &world_a] {
        with_world_services(published, || {
            let a = engine_a.control_bar_bridge().expect("A's catalog");
            assert!(a.find_command_button_by_name(NAME).is_some());
            assert!(engine_b.control_bar_bridge().is_none(), "B never sees A's catalog");
        });
    }
    crate::control_bar::refresh_control_bar_bridge_for_world(&world_b, None).unwrap();
    let b = engine_b.control_bar_bridge().expect("B's catalog");
    assert!(b.find_command_button_by_name(NAME).is_none());
    assert!(!std::sync::Arc::ptr_eq(&b, &engine_a.control_bar_bridge().unwrap()));
}

/// C++ doAdd/RemoveCommandBarButton write the dispatching world's ControlBar
/// and its GameLogic::setControlBarOverride, then markUIDirty that world's
/// bar; CommandSet::getCommandButton consults only its own world's table.
/// Two worlds take turns on ONE thread, each dispatching while the peer is
/// published.
#[test]
fn script_command_bar_override_lands_in_the_dispatching_engines_world() {
    let _lock = crate::test_sync::lock();
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct DirtyHooks(Arc<AtomicUsize>);
    impl crate::control_bar::ControlBarUiHooks for DirtyHooks {
        fn mark_ui_dirty(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn on_player_science_purchase_points_changed(&self, _: i32, _: i32) {}
        fn on_player_rank_changed(&self, _: i32, _: i32, _: i32) {}
    }
    const SET: &str = "EngineOwnedOverrideSet";
    const BUTTON: &str = "Command_EngineOwnedOverride";
    const INI_BUTTON: &str = "Command_EngineOwnedIni";
    let world_a = new_world_services();
    let world_b = new_world_services();
    let dirty_a = Arc::new(AtomicUsize::new(0));
    let dirty_b = Arc::new(AtomicUsize::new(0));
    for (world, dirty) in [(&world_a, &dirty_a), (&world_b, &dirty_b)] {
        {
            let mut bar = world.common().ini.control_bar.write();
            bar.new_command_button(BUTTON.into()).command = "STOP".into();
            bar.new_command_button(INI_BUTTON.into()).command = "STOP".into();
        }
        world
            .common()
            .ini
            .command_set
            .write()
            .new_command_set(SET.into())
            .set_button_at_position(3, INI_BUTTON.into())
            .unwrap();
        crate::control_bar::refresh_control_bar_bridge_for_world(world, None).unwrap();
        *world.control_bar_ui_hooks().lock().unwrap() = Some(Arc::new(DirtyHooks(Arc::clone(dirty))));
    }
    let logic_a = world_a.game_logic_mutex();
    let logic_b = world_b.game_logic_mutex();
    let engine_a = ScriptEngine::new_with_services(&world_a).expect("engine A");
    let engine_b = ScriptEngine::new_with_services(&world_b).expect("engine B");
    let set_override = |engine: &ScriptEngine, slot: usize, button: Option<&str>| {
        let state = std::cell::RefCell::new(ScriptContext::at_frame(1));
        ScriptActionDispatcher::new(engine, &state)
            .set_owning_command_set_slot_override(SET, slot, button)
            .expect("override")
    };
    let slot_button = |engine: &ScriptEngine, slot: usize| {
        engine
            .control_bar_bridge()
            .unwrap()
            .find_command_set_by_name(SET)
            .unwrap()
            .get_command_button(slot)
            .map(|button| button.name.clone())
    };
    // Turn 1: A adds slot 2 while B is published.
    with_world_services(&world_b, || set_override(&engine_a, 2, Some(BUTTON)));
    assert_eq!(slot_button(&engine_a, 2).as_deref(), Some(BUTTON));
    assert_eq!(slot_button(&engine_b, 2), None);
    assert_eq!(
        logic_a.lock().unwrap().find_control_bar_override(SET, 2),
        Some(Some(BUTTON.to_string()))
    );
    assert_eq!(logic_b.lock().unwrap().find_control_bar_override(SET, 2), None);
    assert_eq!((dirty_a.load(Ordering::Relaxed), dirty_b.load(Ordering::Relaxed)), (1, 0));
    // Turn 2: B removes its INI slot 3 while A is published; A keeps its button.
    with_world_services(&world_a, || set_override(&engine_b, 3, None));
    assert_eq!(logic_b.lock().unwrap().find_control_bar_override(SET, 3), Some(None));
    assert_eq!(logic_a.lock().unwrap().find_control_bar_override(SET, 3), None);
    assert_eq!(slot_button(&engine_b, 3), None);
    assert_eq!(slot_button(&engine_a, 3).as_deref(), Some(INI_BUTTON));
    assert_eq!((dirty_a.load(Ordering::Relaxed), dirty_b.load(Ordering::Relaxed)), (1, 1));
    // Turn 3: B's world reloads its catalog while A is published: the rebuilt
    // bar still consults B's own GameLogic table (removed slot stays removed).
    with_world_services(&world_a, || {
        crate::control_bar::refresh_control_bar_bridge_for_world(&world_b, None).unwrap()
    });
    assert_eq!(slot_button(&engine_b, 3), None);
    assert_eq!(slot_button(&engine_a, 3).as_deref(), Some(INI_BUTTON));
}

#[test]
fn radar_visibility_and_border_shroud_follow_the_receiving_world() {
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use game_engine::common::{ini::ini_game_data::GlobalData, system::radar::get_radar_system};

    struct BorderRecorder(Mutex<Vec<u8>>);

    impl ScriptActionHandler for BorderRecorder {
        fn set_border_shroud_level(&self, level: u8) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(level);
            Ok(())
        }
    }

    let receiving = new_world_services();
    let peer = new_world_services();
    for (world, clear_alpha, shroud_alpha) in [(&receiving, 211, 37), (&peer, 97, 55)] {
        let mut config = GlobalData::default();
        config.clear_alpha = clear_alpha;
        config.shroud_alpha = shroud_alpha;
        *world.common().system.global_data.lock().unwrap() =
            Some(Arc::new(parking_lot::RwLock::new(config)));
    }

    let receiving_radar = with_world_services(&receiving, get_radar_system);
    let peer_radar = with_world_services(&peer, get_radar_system);
    let handler = Arc::new(BorderRecorder(Mutex::new(Vec::new())));
    let mut engine = ScriptEngine::new_with_services(&receiving).unwrap();
    engine.set_action_handler(Some(handler.clone()));
    let state = std::cell::RefCell::new(ScriptContext::new());
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);

    with_world_services(&peer, || {
        for kind in [
            ScriptActionType::RadarDisable,
            ScriptActionType::DisableBorderShroud,
            ScriptActionType::EnableBorderShroud,
        ] {
            assert_eq!(
                dispatcher.execute_action(&ScriptAction::new(kind)).unwrap(),
                ScriptActionResult::Success
            );
        }
    });

    assert_eq!(
        (
            receiving_radar.read().unwrap().is_radar_hidden(),
            peer_radar.read().unwrap().is_radar_hidden(),
            handler.0.lock().unwrap().clone(),
        ),
        (true, false, vec![211, 37]),
    );
}
