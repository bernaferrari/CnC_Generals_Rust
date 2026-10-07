//! The legacy facade must resolve to the lexical ScriptEngine owner first.
use super::*;
use crate::helpers::TheScriptEngine;

/// Keep the process slot exactly as this test found it, including an empty slot.
struct ScriptEngineSlotRestore(Option<ScriptEngine>);

impl ScriptEngineSlotRestore {
    fn install(replacement: Option<ScriptEngine>) -> Self {
        let handle = get_script_engine();
        let mut slot = handle.write().expect("script engine slot");
        Self(std::mem::replace(&mut *slot, replacement))
    }
}

impl Drop for ScriptEngineSlotRestore {
    fn drop(&mut self) {
        let handle = get_script_engine();
        let mut slot = handle.write().expect("script engine slot");
        *slot = self.0.take();
    }
}

/// Restore only the standalone bootstrap value. Keep an empty engine slot
/// installed for this guard's full lifetime so restoration cannot mutate a
/// previously installed engine.
struct BootstrapDifficultyRestore(i32);

impl BootstrapDifficultyRestore {
    fn set(value: i32) -> Self {
        let previous = TheScriptEngine::get_global_difficulty();
        TheScriptEngine::set_global_difficulty(value);
        Self(previous)
    }
}

impl Drop for BootstrapDifficultyRestore {
    fn drop(&mut self) {
        TheScriptEngine::set_global_difficulty(self.0);
    }
}

fn engine_with_owner_flag(flag: &str, difficulty: crate::player::GameDifficulty) -> ScriptEngine {
    let mut engine = ScriptEngine::new().expect("script engine");
    engine.set_global_difficulty(difficulty);

    let mut script = Script::new();
    script.script_name = format!("FacadeOwner_{flag}");
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = Some(set_flag_action(flag));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .expect("install owner script");
    engine
}

#[derive(Debug, PartialEq, Eq)]
struct FacadeObservation {
    is_ending: bool,
    time_frozen: bool,
    debug_frozen: bool,
    any_freeze: bool,
    initial_difficulty: i32,
    difficulty: i32,
    owner_flag: bool,
    other_flag: bool,
    ui_interaction: bool,
    video_first: bool,
    video_duplicate: bool,
    video_after_flush: bool,
    explicit_object_change_frame: u32,
    public_object_change_frame: u32,
}

struct FacadeProbeDriver {
    owner_flag: &'static str,
    other_flag: &'static str,
    difficulty: i32,
    observations: Vec<FacadeObservation>,
}

impl ScriptExecutionDriver for FacadeProbeDriver {
    fn after_action(&mut self) -> GameLogicResult<()> {
        // Seed these only after update has begun: a pre-existing end timer or
        // debug freeze can prevent this script's action from running.
        with_script_engine_mut(|engine| {
            engine.start_quick_end_game_timer();
            engine.do_freeze_time();
            engine.lock_inner_mut().freeze_by_debug = true;
        })
        .expect("active owner during update");

        let initial_difficulty = TheScriptEngine::get_global_difficulty();
        let ui_name = format!("facade-ui-{}", self.owner_flag);
        let video_name = format!("facade-video-{}", self.owner_flag);
        TheScriptEngine::signal_ui_interact(&ui_name);
        TheScriptEngine::notify_of_object_count_changed_at_frame(73);
        let explicit_object_change_frame =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
                .expect("active owner can be read after explicit frame notification");
        TheScriptEngine::notify_of_object_creation_or_destruction();
        TheScriptEngine::notify_of_completed_video(&video_name);
        TheScriptEngine::notify_of_completed_video(&video_name);
        TheScriptEngine::set_global_difficulty(self.difficulty);

        let video_first = TheScriptEngine::is_video_complete(&video_name, true);
        let video_duplicate = TheScriptEngine::is_video_complete(&video_name, true);
        let video_after_flush = TheScriptEngine::is_video_complete(&video_name, false);
        let (owner_flag, other_flag, ui_interaction, public_object_change_frame) =
            with_script_engine_ref(|engine| {
                (
                    engine
                        .get_flag(self.owner_flag)
                        .is_some_and(|flag| flag.value),
                    engine
                        .get_flag(self.other_flag)
                        .is_some_and(|flag| flag.value),
                    engine.has_ui_interaction(&ui_name),
                    engine.get_frame_object_count_changed(),
                )
            })
            .expect("active owner can be read");

        self.observations.push(FacadeObservation {
            is_ending: TheScriptEngine::is_game_ending(),
            time_frozen: TheScriptEngine::is_time_frozen_script(),
            debug_frozen: TheScriptEngine::is_time_frozen_debug(),
            any_freeze: TheScriptEngine::is_time_frozen(),
            initial_difficulty,
            difficulty: TheScriptEngine::get_global_difficulty(),
            owner_flag,
            other_flag,
            ui_interaction,
            video_first,
            video_duplicate,
            video_after_flush,
            explicit_object_change_frame,
            public_object_change_frame,
        });
        Ok(())
    }
}

fn drive_owner(
    engine: &mut ScriptEngine,
    owner_flag: &'static str,
    other_flag: &'static str,
    difficulty: i32,
) -> FacadeObservation {
    let mut driver = FacadeProbeDriver {
        owner_flag,
        other_flag,
        difficulty,
        observations: Vec::new(),
    };
    engine
        .update_with_driver(ScriptContext::new(), &mut driver)
        .expect("owner update");
    assert_eq!(
        driver.observations.len(),
        1,
        "one action reaches the driver"
    );
    driver.observations.pop().expect("one owner observation")
}

fn expected_observation(
    owner_flag: bool,
    other_flag: bool,
    initial_difficulty: i32,
    difficulty: i32,
) -> FacadeObservation {
    let public_object_change_frame = crate::helpers::TheGameLogic::get_frame() as u32;
    FacadeObservation {
        is_ending: true,
        time_frozen: true,
        debug_frozen: true,
        any_freeze: true,
        initial_difficulty,
        difficulty,
        owner_flag,
        other_flag,
        ui_interaction: true,
        video_first: true,
        video_duplicate: true,
        video_after_flush: false,
        explicit_object_change_frame: 73,
        public_object_change_frame,
    }
}

#[test]
fn active_facades_follow_interleaved_owners_when_process_slot_is_empty() {
    let _guard = crate::test_sync::lock();
    let _slot = ScriptEngineSlotRestore::install(None);
    let bootstrap_difficulty = TheScriptEngine::get_global_difficulty();
    let _bootstrap = BootstrapDifficultyRestore::set(bootstrap_difficulty);
    let mut first = engine_with_owner_flag("facade-first", crate::player::GameDifficulty::Easy);
    let mut second = engine_with_owner_flag("facade-second", crate::player::GameDifficulty::Brutal);

    assert_eq!(
        drive_owner(&mut first, "facade-first", "facade-second", 2),
        expected_observation(true, false, 0, 2)
    );
    assert_eq!(
        drive_owner(&mut second, "facade-second", "facade-first", 0),
        expected_observation(true, false, 3, 0)
    );

    assert_eq!(
        first.get_global_difficulty(),
        crate::player::GameDifficulty::Hard
    );
    assert_eq!(
        second.get_global_difficulty(),
        crate::player::GameDifficulty::Easy
    );
    assert_eq!(
        TheScriptEngine::get_global_difficulty(),
        bootstrap_difficulty,
        "owner-local writes must not leak into the standalone bootstrap value"
    );
}

#[test]
fn active_facades_ignore_foreign_slot_then_standalone_calls_fall_back_to_it() {
    let _guard = crate::test_sync::lock();
    let _empty_slot = ScriptEngineSlotRestore::install(None);
    let bootstrap_difficulty = TheScriptEngine::get_global_difficulty();
    let _bootstrap = BootstrapDifficultyRestore::set(bootstrap_difficulty);
    let foreign = engine_with_owner_flag("foreign-only", crate::player::GameDifficulty::Easy);
    let _foreign_slot = ScriptEngineSlotRestore::install(Some(foreign));
    let mut driving = engine_with_owner_flag("driving-only", crate::player::GameDifficulty::Brutal);

    assert_eq!(
        drive_owner(&mut driving, "driving-only", "foreign-only", 3),
        expected_observation(true, false, 3, 3)
    );
    {
        let handle = get_script_engine();
        let slot = handle.read().expect("foreign slot");
        let foreign = slot.as_ref().expect("foreign engine remains installed");
        assert_eq!(
            foreign.get_global_difficulty(),
            crate::player::GameDifficulty::Easy
        );
        assert!(!foreign.has_ui_interaction("facade-ui-driving-only"));
        assert!(!foreign.is_video_complete("facade-video-driving-only", false));
        assert_eq!(foreign.get_frame_object_count_changed(), 0);
    }
    assert_eq!(
        TheScriptEngine::get_global_difficulty(),
        bootstrap_difficulty
    );

    // With no lexical owner, the existing standalone facade still targets the
    // process slot, including the C++ one-at-a-time video flush behavior.
    TheScriptEngine::signal_ui_interact("standalone-facade-ui");
    TheScriptEngine::notify_of_completed_video("standalone-facade-video");
    assert!(TheScriptEngine::is_video_complete(
        "standalone-facade-video",
        true
    ));
    assert!(!TheScriptEngine::is_video_complete(
        "standalone-facade-video",
        false
    ));
    let handle = get_script_engine();
    let slot = handle.read().expect("foreign slot");
    let foreign = slot.as_ref().expect("foreign engine remains installed");
    assert!(foreign.has_ui_interaction("standalone-facade-ui"));
}

#[test]
fn active_read_query_fails_closed_during_exclusive_borrow() {
    let _guard = crate::test_sync::lock();
    // Isolate the bootstrap atomic from any test-installed process engine so
    // its differing value cannot be mistaken for this active Normal owner.
    let _empty_slot = ScriptEngineSlotRestore::install(None);
    let _bootstrap = BootstrapDifficultyRestore::set(3);
    let mut foreign = engine_with_owner_flag("foreign-read", crate::player::GameDifficulty::Hard);
    foreign.start_quick_end_game_timer();
    foreign.do_freeze_time();
    let slot = ScriptEngineSlotRestore::install(Some(foreign));
    let driving = ScriptEngine::new().expect("driving engine");

    driving.with_active_for_test(|| {
        let _exclusive = driving.lock_inner_mut();
        assert!(!TheScriptEngine::is_game_ending());
        assert!(!TheScriptEngine::is_time_frozen_script());
        assert_eq!(
            TheScriptEngine::get_global_difficulty(),
            crate::player::GameDifficulty::Normal as i32,
            "an unavailable active owner must not fall through to foreign/bootstrap state"
        );
    });

    drop(slot);
}

#[test]
fn nested_active_facade_restores_outer_owner_after_panic_and_keeps_fallback() {
    let _guard = crate::test_sync::lock();
    let fallback = ScriptEngine::new().expect("fallback engine");
    let _slot = ScriptEngineSlotRestore::install(Some(fallback));
    let outer = ScriptEngine::new().expect("outer engine");
    let inner = ScriptEngine::new().expect("inner engine");

    outer.with_active_for_test(|| {
        TheScriptEngine::signal_ui_interact("outer-before");
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            inner.with_active_for_test(|| {
                TheScriptEngine::signal_ui_interact("inner-before-panic");
                panic!("exercise scoped active-owner restoration");
            });
        }));
        assert!(panic.is_err());
        TheScriptEngine::signal_ui_interact("outer-after");
        assert!(
            with_script_engine_ref(|engine| {
                engine.has_ui_interaction("outer-before")
                    && engine.has_ui_interaction("outer-after")
                    && !engine.has_ui_interaction("inner-before-panic")
            })
            .expect("outer active engine restored")
        );
    });

    assert!(outer.has_ui_interaction("outer-before"));
    assert!(outer.has_ui_interaction("outer-after"));
    assert!(inner.has_ui_interaction("inner-before-panic"));
    TheScriptEngine::signal_ui_interact("standalone-after-panic");
    let handle = get_script_engine();
    let slot = handle.read().expect("fallback slot");
    assert!(
        slot.as_ref()
            .expect("fallback engine remains installed")
            .has_ui_interaction("standalone-after-panic")
    );
}
