//! AIStates.cpp:1290-1446 through the actual factory-cached AI.
use super::*;

#[test]
fn factory_idle_tick_acquires_and_dispatches_attack_in_same_call() {
    if !child(concat!(
        module_path!(),
        "::factory_idle_tick_acquires_and_dispatches_attack_in_same_call"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before_rng = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    assert_eq!(ai.get_next_mood_target_id(false, true), fixture.target_id);
    ai.set_next_mood_check_time(1);
    eprintln!("NATIVE_IDLE_CALLBACK_STAGE before ordinary factory AI update");
    ai.update().expect("ordinary native factory AI update");
    eprintln!("NATIVE_IDLE_CALLBACK_STAGE after ordinary factory AI update");
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32),
        "CPP AIIdle issues its due attack synchronously in this tick"
    );
    assert_eq!(ai.get_current_command(), Some(AiCommandType::AttackObject));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAI);
    assert_eq!(ai.get_goal_object_id(), fixture.target_id);
    assert_eq!(
        ai.get_next_mood_check_time(),
        77,
        "one due 2000 ms / 60-frame idle check"
    );
    assert_eq!(
        get_game_logic_random_seed_state(),
        before_rng,
        "no unrequested randomized mood offset"
    );
    let source = fixture.source.read().unwrap();
    let (weapon, _) = source
        .get_current_weapon()
        .expect("actual installed attack weapon");
    assert_eq!(weapon.max_shot_count, crate::weapon::NO_MAX_SHOTS_LIMIT);
    assert!(!source.ai_pending_goal_none);
    assert!(!source.ai_pending_clear_victim);
    assert!(source.ai_pending_attack_id.is_none());
}

#[test]
fn factory_locked_idle_initializes_before_skipping_acquisition() {
    if !child(concat!(
        module_path!(),
        "::factory_locked_idle_initializes_before_skipping_acquisition"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    ai.set_current_victim(Some(fixture.target_id));
    {
        let native = ai.unit_ai_for_test().unwrap();
        native.data.locomotor_goal_type = 1;
        native
            .ai_state_machine
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .lock();
    }
    ai.update().unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    assert_eq!(ai.get_current_victim(), None);
    assert_eq!(ai.unit_ai_for_test().unwrap().data.locomotor_goal_type, 0);
    assert_eq!(
        ai.get_next_mood_check_time(),
        1,
        "locked Idle must not scan"
    );
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn factory_idle_auto_acquire_mask_preserves_due_timer() {
    if !child(concat!(
        module_path!(),
        "::factory_idle_auto_acquire_mask_preserves_due_timer"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("No");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    ai.set_next_mood_check_time(1);
    ai.update().unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(ai.get_next_mood_check_time(), 1);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn factory_idle_disabled_gates_preserve_due_timer() {
    if !child(concat!(
        module_path!(),
        "::factory_idle_disabled_gates_preserve_due_timer"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    for disabled in [
        crate::common::DisabledType::Paralyzed,
        crate::common::DisabledType::DisabledUnmanned,
        crate::common::DisabledType::DisabledEmp,
        crate::common::DisabledType::DisabledSubdued,
        crate::common::DisabledType::DisabledHacked,
    ] {
        let mut ai = fixture.ai.lock().unwrap();
        // Re-enter so each gate exercises a fresh first-body initialization.
        ai.execute_command(&AiCommandParams::new(
            AiCommandType::Idle,
            CommandSourceType::FromAI,
        ))
        .unwrap();
        drop(ai);
        fixture.source.write().unwrap().set_disabled(disabled);
        let mut ai = fixture.ai.lock().unwrap();
        ai.set_next_mood_check_time(1);
        ai.update().unwrap();
        assert_eq!(
            ai.get_current_state_id(),
            Some(AIStateType::Idle as u32),
            "{disabled:?}"
        );
        assert_eq!(
            ai.get_next_mood_check_time(),
            1,
            "{disabled:?} prevents a scan"
        );
        drop(ai);
        fixture.source.write().unwrap().clear_disabled(disabled);
    }
    // Each explicit Idle entry consumes its one offset; no mood jitter occurs.
    assert_ne!(get_game_logic_random_seed_state(), before);
}

#[test]
fn factory_idle_no_eligible_target_advances_timer_without_attack() {
    if !child(concat!(
        module_path!(),
        "::factory_idle_no_eligible_target_advances_timer_without_attack"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    fixture
        ._factory
        .get_object(fixture.target_id)
        .unwrap()
        .get_base_object()
        .unwrap()
        .write()
        .unwrap()
        .set_status(ObjectStatusTypes::NoAttackFromAi.into(), true);
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    let mut ai = fixture.ai.lock().unwrap();
    assert_eq!(ai.get_next_mood_target_id(false, true), INVALID_ID);
    ai.set_next_mood_check_time(1);
    ai.update().unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
}

#[test]
fn factory_temporary_idle_observes_ordinary_attack_classification() {
    if !child(concat!(
        module_path!(),
        "::factory_temporary_idle_observes_ordinary_attack_classification"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes NOTWHILEATTACKING");
    let mut ai = fixture.ai.lock().unwrap();
    let mut attack =
        AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromPlayer);
    attack.obj = Some(fixture.target_id);
    attack.int_value = 7;
    ai.execute_command(&attack).unwrap();
    ai.set_temporary_state(AIStateType::Idle, 100);
    ai.set_next_mood_check_time(1);
    assert!(ai.is_attacking());
    ai.update().unwrap();
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(ai.get_current_command(), Some(AiCommandType::AttackObject));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(
        ai.get_next_mood_check_time(),
        1,
        "temporary Idle sees the parent's attack veto"
    );
    let native = ai.unit_ai_for_test().unwrap();
    let machine = native.ai_state_machine.as_ref().unwrap().lock().unwrap();
    let before_ordinary = machine
        .temporary_lifecycle_observations()
        .iter()
        .rev()
        .find(|observation| observation.phase == "before_ordinary_update")
        .unwrap();
    assert_eq!(
        before_ordinary.current_victim, None,
        "temporary Idle clears the victim before ordinary Attack updates"
    );
    drop(machine);
    assert_eq!(
        ai.get_current_victim(),
        Some(fixture.target_id),
        "ordinary Attack reacquires after temporary Idle completes"
    );
}

#[test]
fn factory_temporary_idle_dispatches_before_its_continue_return() {
    if !child(concat!(
        module_path!(),
        "::factory_temporary_idle_dispatches_before_its_continue_return"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::super::RestoreAmbientFrame::set(17);
    let fixture = MoodFixture::new("Yes");
    let mut ai = fixture.ai.lock().unwrap();
    let mut attack =
        AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromPlayer);
    attack.obj = Some(fixture.target_id);
    attack.int_value = 7;
    ai.execute_command(&attack).unwrap();
    assert_eq!(ai.get_next_mood_target_id(false, true), fixture.target_id);
    ai.set_temporary_state(AIStateType::Idle, 100);
    ai.set_next_mood_check_time(1);
    let _rng = RestoreRng::set();
    let before = get_game_logic_random_seed_state();
    ai.update().unwrap();
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(ai.get_goal_object_id(), fixture.target_id);
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAI);
    assert_eq!(ai.get_next_mood_check_time(), 77);
    assert_eq!(get_game_logic_random_seed_state(), before);
    let source = fixture.source.read().unwrap();
    assert_eq!(
        source.get_current_weapon().unwrap().0.max_shot_count,
        crate::weapon::NO_MAX_SHOTS_LIMIT
    );
    assert!(!source.ai_pending_goal_none);
    assert!(!source.ai_pending_clear_victim);
}
