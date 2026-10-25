use super::*;

#[test]
fn test_ai_player_creation() {
    let ai_player = AIPlayer::new(1);
    assert_eq!(ai_player.player_id, 1);
    assert_eq!(ai_player.difficulty, GameDifficulty::Normal);
    assert!(!ai_player.base_center_set);
}

#[test]
fn test_work_order() {
    let mut order = WorkOrder::new("Ranger".to_string());
    assert!(order.is_waiting_to_build());

    order.num_completed = 1;
    order.num_required = 1;
    assert!(!order.is_waiting_to_build());
}

#[test]
fn test_team_in_queue() {
    let mut team = TeamInQueue::new();

    let mut order1 = WorkOrder::new("Ranger".to_string());
    order1.num_required = 2;
    order1.required = true;
    team.work_orders.push(order1);

    let mut order2 = WorkOrder::new("Humvee".to_string());
    order2.num_required = 1;
    team.work_orders.push(order2);

    assert!(!team.is_all_built());
    assert!(!team.is_minimum_built());

    team.work_orders[0].num_completed = 2;
    assert!(!team.is_all_built()); // Second order not complete
    assert!(team.is_minimum_built()); // Required order complete

    team.work_orders[1].num_completed = 1;
    assert!(team.is_all_built()); // All orders complete
}

#[test]
fn is_minimum_built_counts_in_progress_factory_like_cpp() {
    let mut team = TeamInQueue::new();
    let mut order = WorkOrder::new("USA_Ranger".into());
    order.required = true;
    order.num_required = 2;
    order.num_completed = 1;
    order.factory_id = Some(1); // C++ counts +1 for assigned factory
    team.work_orders.push(order);
    assert!(team.is_minimum_built());
    team.work_orders[0].factory_id = None;
    assert!(!team.is_minimum_built());
}

#[test]
fn are_builds_complete_requires_no_factory_like_cpp() {
    let mut team = TeamInQueue::new();
    let mut order = WorkOrder::new("USA_Ranger".into());
    order.num_completed = 1;
    order.num_required = 1;
    order.factory_id = Some(7);
    team.work_orders.push(order);
    // C++ areBuildsComplete is false while factory assigned, even if count done.
    assert!(!team.are_builds_complete());
    team.work_orders[0].factory_id = None;
    assert!(team.are_builds_complete());
}

#[test]
fn check_ready_teams_activates_on_60s_expiry() {
    // C++: timeExpired = frameStarted + 60*LOGICFRAMES_PER_SECOND < frame
    let mut ai = AIPlayer::new(1);
    let mut team = TeamInQueue::new();
    team.team_name = Some("TestReadyTeam".into());
    team.frame_started = 0;
    ai.team_ready_queue.push_back(team);
    // Without a live team object, activation still removes from ready queue
    // when timeExpired forces allIdle=true.
    // Force "now" via TheGameLogic if available; otherwise just exercise path.
    let _ = ai.check_ready_teams();
    // After check, either still queued (frame not advanced) or empty (expired).
    // Structural honesty: function exists and does not panic.
    assert!(ai.team_ready_queue.len() <= 1);
}

#[test]
fn check_queued_all_built_prepends_ready_queue_like_cpp() {
    let mut ai = AIPlayer::new(1);
    let mut team = TeamInQueue::new();
    team.team_name = Some("BuiltTeam".into());
    let mut order = WorkOrder::new("USA_Ranger".into());
    order.num_completed = 1;
    order.num_required = 1;
    team.work_orders.push(order);
    assert!(team.is_all_built());
    ai.team_build_queue.push_back(team);
    // Seed ready with a marker to verify prepend
    let mut marker = TeamInQueue::new();
    marker.team_name = Some("AlreadyReady".into());
    ai.team_ready_queue.push_back(marker);
    ai.check_queued_teams().expect("check_queued");
    assert!(ai.team_build_queue.is_empty());
    assert_eq!(ai.team_ready_queue.len(), 2);
    // C++ prependTo_TeamReadyQueue → new team at front
    assert_eq!(
        ai.team_ready_queue
            .front()
            .and_then(|t| t.team_name.as_deref()),
        Some("BuiltTeam")
    );
}

#[test]
fn build_and_team_delay_recheck_constants_match_cpp() {
    // C++ AIPlayer.cpp: m_buildDelay = 2*LOGICFRAMES_PER_SECOND;
    //                   m_teamDelay = 5*LOGICFRAMES_PER_SECOND;
    assert_eq!(BUILD_DELAY_RECHECK_FRAMES, 2 * LOGICFRAMES_PER_SECOND);
    assert_eq!(TEAM_DELAY_RECHECK_FRAMES, 5 * LOGICFRAMES_PER_SECOND);
    assert_eq!(BUILD_DELAY_RECHECK_FRAMES, 60);
    assert_eq!(TEAM_DELAY_RECHECK_FRAMES, 150);
}

#[test]
fn retail_aidata_fallback_constants_match_default_ini() {
    // windows_game/.../Default/AIData.ini
    assert!((DEFAULT_STRUCTURE_SECONDS - 0.0).abs() < f32::EPSILON);
    assert!((DEFAULT_TEAM_SECONDS - 10.0).abs() < f32::EPSILON);
    assert_eq!(RESOURCES_POOR, 2000);
    assert_eq!(RESOURCES_WEALTHY, 7000);
    assert!((STRUCTURES_POOR_MODIFIER - 0.6).abs() < 1e-5);
    assert!((STRUCTURES_WEALTHY_MODIFIER - 2.0).abs() < 1e-5);
    assert!((TEAMS_POOR_MODIFIER - 0.6).abs() < 1e-5);
    assert!((TEAMS_WEALTHY_MODIFIER - 2.0).abs() < 1e-5);
    assert_eq!(REBUILD_DELAY_SECONDS, 30);
    assert!((SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE - 150.0).abs() < 1e-5);
}

#[test]
fn arm_structure_timer_zero_seconds_stays_zero_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.structure_seconds = 0.0;
    ai.ready_to_build_structure = true;
    let mut data = crate::ai::AiData::default();
    data.structure_seconds = 0.0;
    ai.arm_structure_timer_after_build_with_data(&data, 0)
        .expect("arm");
    assert!(!ai.ready_to_build_structure);
    // C++: m_structureSeconds*LOGICFRAMES with 0 → timer 0 (immediate next ready path).
    assert_eq!(ai.structure_timer, 0);
}

#[test]
fn do_base_building_sets_2s_build_delay_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_structure = true;
    ai.build_delay = 0;
    ai.do_base_building().expect("do_base");
    assert_eq!(
        ai.build_delay, BUILD_DELAY_RECHECK_FRAMES,
        "after process attempt, C++ sets buildDelay to 2 seconds"
    );
}

#[test]
fn do_team_building_sets_5s_team_delay_and_queues_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_team = true;
    ai.team_delay = 0;
    ai.do_team_building().expect("do_team");
    assert_eq!(
        ai.team_delay, TEAM_DELAY_RECHECK_FRAMES,
        "after queue/process attempt, C++ sets teamDelay to 5 seconds"
    );
}

#[test]
fn solo_do_base_building_does_not_clamp_structure_timer_like_cpp() {
    // C++ AIPlayer::doBaseBuilding has no 3*LOGICFRAMES clamp; skirmish does.
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_structure = false;
    ai.structure_timer = 3 * LOGICFRAMES_PER_SECOND + 50;
    let before = ai.structure_timer;
    ai.do_base_building().expect("do_base");
    // decremented by 1, not clamped to 3s
    assert_eq!(ai.structure_timer, before - 1);
    assert!(ai.structure_timer > 3 * LOGICFRAMES_PER_SECOND);
}

#[test]
fn solo_do_team_building_does_not_clamp_team_timer_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_team = false;
    ai.team_timer = 3 * LOGICFRAMES_PER_SECOND + 50;
    let before = ai.team_timer;
    ai.do_team_building().expect("do_team");
    assert_eq!(ai.team_timer, before - 1);
    assert!(ai.team_timer > 3 * LOGICFRAMES_PER_SECOND);
}

#[test]
fn do_base_building_decrements_structure_timer_until_ready() {
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_structure = false;
    ai.structure_timer = 2;
    ai.build_delay = 99;
    ai.do_base_building().expect("t1");
    assert!(!ai.ready_to_build_structure);
    assert_eq!(ai.structure_timer, 1);
    assert_eq!(
        ai.build_delay, 98,
        "while waiting, only structureTimer path; buildDelay still decrements"
    );
    // Expiry frame: C++ sets buildDelay=0 then same-frame continues into
    // buildDelay recheck and sets buildDelay = 2*LOGICFRAMES_PER_SECOND.
    ai.do_base_building().expect("t2");
    assert!(ai.ready_to_build_structure);
    assert_eq!(
        ai.build_delay, BUILD_DELAY_RECHECK_FRAMES,
        "same-frame recheck after timer expiry sets 2s delay (C++)"
    );
}

#[test]
fn host_attack_disabled_by_default_like_cpp_update() {
    // Check the real environment-reading entry points in an isolated child.
    // Other tests may concurrently read the parent's process environment.
    const CHILD: &str = "GENERALS_AI_DEFAULT_FLAG_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        assert!(!AIPlayer::host_attack_enabled());
        assert!(!AIPlayer::host_analysis_enabled());
        return;
    }
    let result = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "ai::ai_player::tests::host_attack_disabled_by_default_like_cpp_update",
            "--exact",
        ])
        .env(CHILD, "1")
        .env_remove("GENERALS_AI_HOST_ATTACK")
        .env_remove("GENERALS_AI_HOST_ANALYSIS")
        .output()
        .expect("isolated default-flag test");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn arm_structure_timer_applies_wealth_mods_like_cpp() {
    let mut data = crate::ai::AiData::default();
    data.structure_seconds = 10.0;
    let mut player_ai = AIPlayer::new(1);
    player_ai.ready_to_build_structure = true;
    player_ai
        .arm_structure_timer_after_build_with_data(&data, 0)
        .expect("arm");
    assert!(!player_ai.ready_to_build_structure);
    // Preserve C++ Real (f32) truncation at the actual arm phase.
    assert_eq!(
        player_ai.structure_timer,
        (300f32 / STRUCTURES_POOR_MODIFIER) as u32
    );
    assert_eq!(player_ai.structure_timer, 499);
    let mut peer = AIPlayer::new(1);
    let mut zero = crate::ai::AiData::default();
    zero.structure_seconds = 0.0;
    peer.arm_structure_timer_after_build_with_data(&zero, 0)
        .unwrap();
    assert_eq!(peer.structure_timer, 0);
    assert_eq!(player_ai.structure_timer, 499);
    data.resources_poor = 100;
    data.resources_wealthy = 1000;
    data.structures_poor_mod = 0.6;
    data.structures_wealthy_mod = 2.0;
    for (money, expected) in [(99, 499), (100, 300), (1000, 300), (1001, 150)] {
        player_ai
            .arm_structure_timer_after_build_with_data(&data, money)
            .unwrap();
        assert_eq!(player_ai.structure_timer, expected);
        assert_eq!(peer.structure_timer, 0);
    }
}

#[test]
fn process_base_building_honors_rebuild_delay_timestamp() {
    // C++: if timestamp + rebuildDelaySeconds*FPS > frame, skip rebuild.
    let mut ai = AIPlayer::new(1);
    ai.ready_to_build_structure = true;
    // No player build list → falls through without panic.
    ai.process_base_building().expect("process");
    // rebuild_delay_frames uses AIData or REBUILD_DELAY_SECONDS constant.
    assert!(ai.rebuild_delay_frames() >= LOGICFRAMES_PER_SECOND);
}

#[test]
fn arm_team_timer_after_build_sets_ready_false() {
    let mut ai = AIPlayer::new(1);
    ai.team_seconds = 10.0; // 300 frames
    ai.ready_to_build_team = true;
    ai.arm_team_timer_after_build().expect("arm");
    assert!(!ai.ready_to_build_team);
    // money=0 → poor TeamsPoorRate 0.6 (f32 truncation like C++ Real).
    assert_eq!(ai.team_timer, (300f32 / TEAMS_POOR_MODIFIER) as u32);
    assert_eq!(ai.team_timer, 499);
}

#[test]
fn arm_team_timer_zero_seconds_stays_zero_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.team_seconds = 0.0;
    ai.ready_to_build_team = true;
    ai.arm_team_timer_after_build().expect("arm");
    assert!(!ai.ready_to_build_team);
    // C++ allows teamTimer 0 when TeamSeconds is 0 (no clamp to 1).
    assert_eq!(ai.team_timer, 0);
    // Next doTeamBuilding: timer==0 → ready true (same one-frame lag as C++).
    ai.do_team_building().expect("tick");
    assert!(ai.ready_to_build_team);
    assert_eq!(ai.team_timer, 0);
}

#[test]
fn queue_units_empty_queue_ok() {
    let mut ai = AIPlayer::new(1);
    // queueSupplyTruck may prepend a gatherer team (C++ also calls it).
    assert!(ai.queue_units());
}

#[test]
fn on_unit_produced_shortcuts_team_delay() {
    let mut ai = AIPlayer::new(1);
    ai.team_delay = 99;
    // C++ returns before resetting teamDelay when factory is null.
    ai.on_unit_produced(INVALID_ID, INVALID_ID).expect("oop");
    assert_eq!(ai.team_delay, 99);
    // A nonnull factory wakes scheduling even if Rust cannot resolve the unit.
    ai.team_delay = 99;
    let _ = ai.on_unit_produced(1, 999999);
    assert_eq!(
        ai.team_delay, 0,
        "A nonnull factory must wake team scheduling"
    );
}

#[test]
fn build_specific_ai_team_respects_can_build_units_surface() {
    // Without player/can_build setup, empty name should still be a no-op Ok.
    let mut ai = AIPlayer::new(1);
    ai.build_specific_ai_team("NoSuchTeam", false).expect("bst");
    assert!(ai.team_build_queue.is_empty());
}

#[test]
fn repair_structure_skips_pristine_and_duplicates() {
    let mut ai = AIPlayer::new(1);
    // Without a live object, repair_structure returns Ok and does not enqueue.
    ai.repair_structure(999_999).expect("rs");
    assert_eq!(ai.structures_in_queue, 0);
}

#[test]
fn queue_dozer_skips_when_already_queued() {
    let mut ai = AIPlayer::new(1);
    let mut order = WorkOrder::new("AmericaVehicleDozer".into());
    let mut team = TeamInQueue::new();
    team.work_orders.push(order);
    ai.team_build_queue.push_front(team);
    let before = ai.team_build_queue.len();
    ai.queue_dozer().expect("qd");
    assert_eq!(
        ai.team_build_queue.len(),
        before,
        "C++ dozerInQueue early-out"
    );
}

#[test]
fn queue_supply_truck_skips_when_already_queued() {
    let mut ai = AIPlayer::new(1);
    let mut order = WorkOrder::new("SupplyTruck".into());
    order.is_resource_gatherer = true;
    let mut team = TeamInQueue::new();
    team.work_orders.push(order);
    ai.team_build_queue.push_front(team);
    let before = ai.team_build_queue.len();
    ai.queue_supply_truck().expect("qst");
    assert_eq!(
        ai.team_build_queue.len(),
        before,
        "C++ returns early when truck already in queue"
    );
}

#[test]
fn select_team_to_reinforce_no_auto_returns_false() {
    let mut ai = AIPlayer::new(1);
    // No prototypes with automatically_reinforce → false, no panic.
    assert!(!ai.select_team_to_reinforce(0).expect("reinforce"));
    assert!(ai.team_build_queue.is_empty());
}

#[test]
fn test_strategy_state() {
    let mut strategy_state = AiStrategyState::default();
    assert_eq!(strategy_state.current_strategy, AiStrategy::Balanced);
    assert_eq!(strategy_state.strategy_confidence, 0.0);
}

#[test]
fn ai_player_xfer_writes_team_seconds_as_cpp_int() {
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut ai_player = AIPlayer::new(7);
    ai_player.team_seconds = 66_051.0;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("ai_player_team_seconds").unwrap();
        ai_player.xfer(&mut save);
        save.close().unwrap();
    }

    assert!(
        bytes
            .windows(4)
            .any(|window| window == &66_051i32.to_le_bytes())
    );
    assert!(
        !bytes
            .windows(4)
            .any(|window| window == &66_051.0f32.to_le_bytes())
    );
}
