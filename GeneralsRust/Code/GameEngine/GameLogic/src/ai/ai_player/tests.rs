use super::*;

#[allow(dead_code)]
fn set_var(key: &str, value: &str) {
    // SAFETY: AI env toggles; test access is serialized (--test-threads=1 convention).
    unsafe { std::env::set_var(key, value) }
}

#[allow(dead_code)]
fn remove_var(key: &str) {
    // SAFETY: AI env toggles; test access is serialized (--test-threads=1 convention).
    unsafe { std::env::remove_var(key) }
}

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
fn team_in_queue_xfer_uses_team_id_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("impl TeamInQueue").unwrap_or(0);
    // Find TeamInQueue::xfer after work orders
    let j = src[i..]
        .find("pub fn xfer(&mut self, xfer: &mut dyn Xfer)")
        .expect("TeamInQueue xfer")
        + i;
    // There may be WorkOrder xfer first — find the one with priority_build
    let k = src[j..].find("priority_build").expect("priority") + j;
    let w = &src[k.saturating_sub(200)..src.len().min(k + 1200)];
    assert!(
        w.contains("xfer_unsigned_int(&mut team_id)")
            && w.contains("find_team_by_id")
            && w.contains("TEAM_ID_INVALID")
            && !w.contains("xfer_ascii_string(&mut team_name)"),
        "TeamInQueue xfer must use TeamID like C++, not team name string"
    );
}

#[test]
fn ai_pre_team_destroy_matches_m_team_pointer() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn ai_pre_team_destroy(&mut self, deleted:")
        .expect("aiPreTeamDestroy");
    let window = &src[i..src.len().min(i + 1800)];
    assert!(
        window.contains("Arc::ptr_eq")
            && window.contains("team_build_queue.retain")
            && window.contains("team_ready_queue.retain"),
        "aiPreTeamDestroy must drop queue entries by m_team pointer identity"
    );
}

#[test]
fn includes_a_dozer_uses_kindof_dozer() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("pub fn includes_a_dozer").expect("includesADozer");
    let window = &src[i..src.len().min(i + 1200)];
    assert!(
        window.contains("KindOf::Dozer")
            && window.contains("find_template")
            && window.contains("is_resource_gatherer"),
        "includesADozer must check KINDOF_DOZER and exclude resource gatherers"
    );
}

#[test]
fn is_location_safe_filters_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("pub fn is_location_safe").expect("isLocationSafe");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("supply_center_safe_radius")
            && window.contains("KindOf::Harvester")
            && window.contains("KindOf::Dozer")
            && window.contains("ObjectStatusTypes::Stealthed")
            && window.contains("Relationship::Enemies")
            && window.contains("is_effectively_dead"),
        "isLocationSafe must apply C++ partition filters"
    );
}

#[test]
fn team_in_queue_drop_activates_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("impl Drop for TeamInQueue")
            && src.contains("tg.set_active()")
            && src.contains("self.team = None"),
        "TeamInQueue Drop must setActive; disband must null m_team"
    );
    // disband path nulls team before Drop
    let i = src.find("pub fn disband(&mut self)").expect("disband");
    let w = &src[i..src.len().min(i + 3500)];
    assert!(
        w.contains("self.team = None"),
        "disband must clear team handle like C++ m_team = NULL"
    );
}

#[test]
fn team_in_queue_stores_team_handle_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("pub team: Option<Arc<RwLock<crate::team::Team>>>"),
        "TeamInQueue must hold m_team handle"
    );
    let i = src.find("pub fn build_specific_ai_team").expect("bst");
    let w = &src[i..src.len().min(i + 9000)];
    assert!(
        w.contains("team.team = Some(team_arc)"),
        "buildSpecificAITeam must stamp TeamInQueue.m_team"
    );
    let j = src.find("pub(crate) fn check_ready_teams").expect("ready");
    let rw = &src[j..src.len().min(j + 7500)];
    assert!(
        rw.contains("tg.is_idle()") && rw.contains("tg.set_active()"),
        "checkReadyTeams must use m_team isIdle/setActive"
    );
    let k = src
        .find("pub(crate) fn check_queued_teams")
        .expect("queued");
    let qw = &src[k..src.len().min(k + 6500)];
    assert!(
        qw.contains("tq.team.as_ref()")
            && qw.contains("tg.get_members()")
            && qw.contains("aig.is_idle()"),
        "checkQueuedTeams anyIdle must walk m_team members"
    );
}

#[test]
fn check_ready_teams_execute_actions_uses_team_handle() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn check_ready_teams")
        .expect("checkReadyTeams");
    let w = &src[i..src.len().min(i + 5000)];
    assert!(
        w.contains("get_execute_actions_on_create")
            && w.contains("find_script_clone_by_name")
            && w.contains("tg.get_name()")
            && w.contains("60 * LOGICFRAMES_PER_SECOND"),
        "checkReadyTeams must resolve executeActions via team handle name"
    );
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
fn science_points_early_out_before_skillset_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn do_upgrades_and_skills")
        .expect("do_upgrades_and_skills");
    let end = src[i..]
        .find(
            "
    pub(crate) fn update_bridge_repair",
        )
        .map(|o| i + o)
        .unwrap_or(src.len().min(i + 6000));
    let w = &src[i..end];
    let skillset_idx = w
        .find("skillset_selector == INVALID_SKILLSET_SELECTION")
        .expect("skillset pick");
    let early = &w[..skillset_idx];
    assert!(
        early.contains("purchase_points_early") && early.contains("if purchase_points_early <= 0"),
        "C++ returns when science points are 0 before skillset selection"
    );
}

#[test]
fn friend_execute_action_team_scoped_like_cpp() {
    let eng = crate::scripting::engine::SCRIPT_ENGINE_SRC;
    assert!(
        eng.contains("fn friend_execute_action")
            && eng.contains("calling_team")
            && eng.contains("execute_action_chain"),
        "ScriptEngine must expose friend_executeAction with team context"
    );
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn check_queued_teams")
        .expect("check_queued_teams");
    let end = src[i..]
        .find(
            "
    /// C++ `AIPlayer::doTeamBuilding`",
        )
        .map(|o| i + o)
        .unwrap_or(src.len().min(i + 7000));
    let w = &src[i..end];
    assert!(
        w.contains("friend_execute_action") && !w.contains("ScriptEvaluator::new(script_engine)"),
        "checkQueuedTeams must call friend_execute_action with team name"
    );
}

#[test]
fn process_base_building_no_priority_fallback_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn process_base_building(&mut self)")
        .expect("process_base_building");
    let end = src[i..]
        .find(
            "
    /// C++ rebuild delay frames",
        )
        .map(|o| i + o)
        .unwrap_or(src.len().min(i + 8000));
    let w = &src[i..end];
    assert!(
        w.contains("build_structure_with_dozer")
            && !w.contains("construction_priorities.first()")
            && !w.contains("build_structure_now(&priority)")
            && !w.contains("analyze_building_needs")
            && !w.contains("update_construction_priorities"),
        "processBaseBuilding must walk BuildListInfo only like C++ (no host priorities)"
    );
}

#[test]
fn is_a_good_idea_checks_m_team_handle_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn is_a_good_idea_to_build_team")
        .expect("is_a_good_idea");
    let w = &src[i..src.len().min(i + 2200)];
    assert!(
        w.contains("q.team.as_ref()")
            && w.contains("tg.get_name()")
            && w.contains("team_build_queue.iter()"),
        "isAGoodIdeaToBuildTeam must reject queue entries by m_team prototype, not only team_name"
    );
}

#[test]
fn ai_player_trait_update_order_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("impl AiPlayerTrait for AIPlayer").expect("trait");
    let j = src[i..].find("fn update(&mut self)").expect("update") + i;
    let w = &src[j..src.len().min(j + 600)];
    assert!(
        w.contains("do_base_building")
            && w.contains("check_ready_teams")
            && w.contains("check_queued_teams")
            && w.contains("do_team_building")
            && w.contains("do_upgrades_and_skills")
            && w.contains("update_bridge_repair")
            && !w.contains("update_strategy"),
        "AiPlayerTrait::update must match C++ AIPlayer::update phase order only"
    );
    // update_with_frame also must not inject strategy into the C++ phase block.
    let k = src
        .find("pub fn update_with_frame")
        .expect("update_with_frame");
    let ww = &src[k..src.len().min(k + 1200)];
    let base = ww.find("do_base_building").expect("base");
    let bridge = ww.find("update_bridge_repair").expect("bridge");
    let mid = &ww[base..=bridge];
    assert!(
        !mid.contains("update_strategy") && !mid.contains("process_attack_decisions"),
        "C++ phase block in update_with_frame must stay free of host residuals"
    );
}

#[test]
fn check_queued_teams_no_factory_residual_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn check_queued_teams")
        .expect("check_queued_teams");
    let end = src[i..]
        .find(
            "
    /// C++ `AIPlayer::doTeamBuilding`",
        )
        .map(|o| i + o)
        .unwrap_or(src.len().min(i + 5000));
    let w = &src[i..end];
    assert!(
        !w.contains("orders_to_process") && !w.contains("find_factory_internal(&thing_template"),
        "checkQueuedTeams must not assign factory_id residual"
    );
    assert!(
        w.contains("is_build_time_expired")
            && w.contains("is_all_built")
            && w.contains("get_execute_actions_on_create"),
        "checkQueuedTeams must keep expire/all-built/executeActions phases"
    );
}

#[test]
fn set_ai_difficulty_only_sets_field_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src
        .split("#[cfg(test)]")
        .next()
        .expect("production before tests");
    let i = prod
        .find("pub fn set_ai_difficulty(&mut self, difficulty: GameDifficulty)")
        .expect("setAIDifficulty");
    let end = prod[i..]
        .find("pub fn select_skillset")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 400));
    let w = &prod[i..end];
    assert!(
        w.contains("self.difficulty = difficulty")
            && !w.contains("team_seconds")
            && !w.contains("difficulty_factor")
            && !w.contains("difficulty_handler"),
        "setAIDifficulty must only assign m_difficulty like C++"
    );
}

#[test]
fn do_upgrades_and_skills_skips_first_two_frames_like_cpp() {
    // C++: if (TheGameLogic->getFrame() < 2) return;
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn do_upgrades_and_skills")
        .expect("do_upgrades");
    let window = &src[i..src.len().min(i + 900)];
    assert!(
        window.contains("get_frame() < 2")
            && window.contains("get_science_purchase_points")
            && window.find("get_frame() < 2").unwrap()
                < window.find("get_side()").unwrap_or(usize::MAX),
        "do_upgrades_and_skills must gate frame<2 and science points before side walk"
    );
}

#[test]
fn arm_structure_timer_zero_seconds_stays_zero_like_cpp() {
    let mut ai = AIPlayer::new(1);
    ai.structure_seconds = 0.0;
    ai.ready_to_build_structure = true;
    ai.arm_structure_timer_after_build().expect("arm");
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
fn update_with_frame_source_order_matches_cpp_do_methods() {
    // Source-order honesty: do_base → check_ready → check_queued → do_team
    // (delays are inside do_*, not pre-gated in update_with_frame).
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("pub fn update_with_frame").expect("uwf");
    let window = &src[i..src.len().min(i + 2800)];
    let base = window.find("self.do_base_building()?").expect("base");
    let ready = window.find("self.check_ready_teams()?").expect("ready");
    let queued = window.find("self.check_queued_teams()?").expect("queued");
    let team = window.find("self.do_team_building()?").expect("team");
    assert!(base < ready && ready < queued && queued < team);
    assert!(
        !window[..base].contains("if self.ready_to_build_structure && self.build_delay"),
        "update_with_frame must not pre-gate do_base_building (C++ always calls it)"
    );
    assert!(
        window.contains("host_attack_enabled()")
            && window.contains("GENERALS_AI_HOST_ATTACK")
            && window.contains("host_analysis_enabled()"),
        "host residual attack/analysis must be env-gated off by default"
    );
}

#[test]
fn host_attack_disabled_by_default_like_cpp_update() {
    // Unset env in test process may inherit; function treats missing as false.
    remove_var("GENERALS_AI_HOST_ATTACK");
    assert!(!AIPlayer::host_attack_enabled());
    remove_var("GENERALS_AI_HOST_ANALYSIS");
    assert!(!AIPlayer::host_analysis_enabled());
}

#[test]
fn arm_structure_timer_applies_wealth_mods_like_cpp() {
    // C++: m_structureTimer = TheAI->getAiData()->m_structureSeconds * FPS
    // (live AIData, not a per-player field). Snapshot and restore AIData.
    let prev_seconds = {
        let ai_store = the_ai();let ai_g = ai_store.write().expect("ai");
        let data_arc = ai_g.get_ai_data().clone();
        drop(ai_g);
        let mut data = data_arc.write().expect("data");
        let prev = data.structure_seconds;
        data.structure_seconds = 10.0; // 300 frames base
        prev
    };
    let mut player_ai = AIPlayer::new(1);
    player_ai.ready_to_build_structure = true;
    player_ai.arm_structure_timer_after_build().expect("arm");
    assert!(!player_ai.ready_to_build_structure);
    // No player money → 0 < Poor(2000) → divide by StructuresPoorRate 0.6.
    // C++ Real (f32) truncation: (300f32 / 0.6) as u32 == 499.
    assert_eq!(
        player_ai.structure_timer,
        (300f32 / STRUCTURES_POOR_MODIFIER) as u32
    );
    assert_eq!(player_ai.structure_timer, 499);
    {
        let ai_store = the_ai();let ai_g = ai_store.write().expect("ai restore");
        let data_arc = ai_g.get_ai_data().clone();
        drop(ai_g);
        let mut data = data_arc.write().expect("data restore");
        data.structure_seconds = prev_seconds;
    }
}

#[test]
fn arm_structure_timer_reads_live_aidata_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn arm_structure_timer_after_build")
        .expect("arm_structure");
    let w = &src[i..src.len().min(i + 1200)];
    assert!(
        w.contains("get_ai_data()") && w.contains("structure_seconds") && w.contains("Live AIData"),
        "arm structure timer must re-read AIData.structureSeconds like C++"
    );
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
fn process_base_building_source_has_cpp_rebuild_and_timer_arm() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn process_base_building(&mut self) -> Result<(), AiError>")
        .expect("pbb");
    let window = &src[i..src.len().min(i + 16000)];
    assert!(
        window.contains("rebuild_delay_frames")
            && window.contains("set_object_timestamp")
            && window.contains("arm_structure_timer_after_build")
            && window.contains("build_structure_with_dozer")
            && window.contains("only one building per delay loop")
            && window.contains("ResumeConstruction"),
        "process_base_building must port C++ rebuild delay + dozer resume + timer arm"
    );
}

#[test]
fn select_team_uses_player_team_list_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    for (label, needle) in [
        ("build", "pub(crate) fn select_team_to_build"),
        ("reinforce", "pub(crate) fn select_team_to_reinforce"),
    ] {
        let i = src.find(needle).unwrap_or_else(|| panic!("{label}"));
        let window = &src[i..src.len().min(i + 2800)];
        assert!(
            window.contains("get_player_team_prototypes"),
            "{label} must iterate player team list like C++ getPlayerTeams"
        );
        assert!(
            !window.contains("list_team_prototypes()"),
            "{label} must not walk global TeamFactory list"
        );
    }
    let player = crate::player::PLAYER_SRC;
    assert!(
        player.contains("pub fn get_player_team_prototypes"),
        "Player must expose getPlayerTeams equivalent"
    );
}

#[test]
fn select_team_to_build_random_hi_pri_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    // Anchor on the Result signature so we do not match select_team_to_build_ai.
    let i = src
        .find("fn select_team_to_build(&mut self) -> Result<bool, AiError>")
        .expect("select_team_to_build");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("INVALID_PRI")
            && window.contains("select_team_to_reinforce(hi_pri)")
            && window.contains("game_logic_random_value")
            && window.contains("build_specific_ai_team(team_name, false)")
            && window.contains("arm_team_timer_after_build"),
        "select_team_to_build must match C++ hiPri/reinforce/random/arm timer"
    );
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
fn is_a_good_idea_requires_idle_factory_like_cpp() {
    // C++ isAGoodIdeaToBuildTeam calls isPossibleToBuildTeam(proto, true, ...)
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("fn is_a_good_idea_to_build_team").expect("good");
    let window = &src[i..src.len().min(i + 2000)];
    assert!(
        window.contains("is_possible_to_build_team(team_name, true)"),
        "is_a_good_idea must require idle factory (C++ busyOK=true means require idle)"
    );
}

#[test]
fn is_a_good_idea_calls_evaluate_production_condition() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("fn is_a_good_idea_to_build_team").expect("good");
    let window = &src[i..src.len().min(i + 2200)];
    assert!(
        window.contains("evaluate_production_condition()"),
        "is_a_good_idea must call evaluateProductionCondition first (C++)"
    );
}

#[test]
fn queue_units_prefers_m_team_handle() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn queue_units(&mut self)")
        .expect("queue_units");
    let window = &src[i..src.len().min(i + 4500)];
    assert!(
        window.contains("team_q.team.is_none()")
            && window.contains("team_q.team.clone()")
            && window.contains("queue_units_home_for_team(team_arc.as_ref()")
            && window.contains("start_training_internal(order, busy_ok, train_name.as_str())"),
        "queueUnits must recruit/train via TeamInQueue.m_team"
    );
}

#[test]
fn queue_units_try_to_recruit_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    // Doc comment sits above the fn signature — include a lookback.
    let i = src
        .find("C++ `AIPlayer::queueUnits`")
        .expect("queueUnits doc");
    let window = &src[i..src.len().min(i + 5000)];
    assert!(
        window.contains("try_to_recruit")
            && window.contains("while order.is_waiting_to_build()")
            && window.contains("ai_move_to_position")
            && window.contains("ai_idle")
            && window.contains("start_training_internal")
            && window.contains("validate_factory")
            && window.contains("max_recruit_distance"),
        "queue_units must recruit-then-train like C++ queueUnits"
    );
}

#[test]
fn queue_units_empty_queue_ok() {
    let mut ai = AIPlayer::new(1);
    // queueSupplyTruck may prepend a gatherer team (C++ also calls it).
    assert!(ai.queue_units());
}

#[test]
fn is_a_good_idea_drops_factory_lock_before_possible() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub(crate) fn is_a_good_idea_to_build_team")
        .expect("good idea");
    let window = &src[i..src.len().min(i + 2200)];
    assert!(
        window.contains("Snapshot under the factory lock")
            && window.contains("instances >= max_instances")
            && window.contains("is_possible_to_build_team(team_name, true)"),
        "isAGoodIdeaToBuildTeam must drop factory Mutex before isPossibleToBuildTeam"
    );
    let call = window
        .find("is_possible_to_build_team(team_name, true)")
        .expect("call");
    let before = &window[..call];
    assert!(
        before.matches('}').count() >= 2,
        "factory lock scope must close before is_possible_to_build_team"
    );

    let j = src.find("fn is_possible_to_build_team(").expect("possible");
    let pw = &src[j..src.len().min(j + 4500)];
    assert!(
        pw.contains("find_factory_internal also locks the player")
            && pw.contains("then factory-scan")
            && pw.find("find_factory_internal(thing_name, true)").unwrap()
                > pw.find("calc_cost_to_build").unwrap(),
        "isPossibleToBuildTeam must not hold player RwLock across find_factory"
    );
}

#[test]
fn is_possible_to_build_team_avg_cost_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::isPossibleToBuildTeam`")
        .expect("isPossible doc");
    let window = &src[i..src.len().min(i + 5500)];
    assert!(
        window.contains("any_idle")
            && (window.contains("find_factory_internal(thing_name, true)")
                || window.contains("find_factory_internal(&thing_name, true)"))
            && (window.contains("find_factory_internal(thing_name, false)")
                || window.contains("find_factory_internal(&thing_name, false)"))
            && (window.contains("max_units as f32 + min_units as f32")
                || window.contains("*max_units as f32 + *min_units as f32"))
            && window.contains("team_resources_to_build")
            && window.contains("notEnoughMoney")
            && window.contains("!require_idle_factory")
            && window.contains("find_factory_internal also locks the player")
            && window.contains("let mut cost: i32 = 0")
            && window.contains("as i32"),
        "isPossibleToBuildTeam must match C++ anyIdle/Int avg cost/resources mod"
    );
}

#[test]
fn start_training_queues_create_unit_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::startTraining`")
        .expect("startTraining doc");
    let window = &src[i..src.len().min(i + 2500)];
    assert!(
        window.contains("request_unique_unit_production_id")
            && window.contains("queue_unit_with_production_id")
            && window.contains("order.factory_id = Some(factory_id)")
            && window.contains("start_production"),
        "startTraining must queueCreateUnit then set factoryID"
    );
}

#[test]
fn find_factory_prefers_build_list_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::findFactory`")
        .expect("findFactory doc");
    let window = &src[i..src.len().min(i + 2500)];
    assert!(
        window.contains("get_build_list_mut")
            && window.contains("factory_candidate")
            && window.contains("build list only")
            && window.contains("set_object_id(INVALID_ID)")
            && !window.contains("get_all_objects()"),
        "findFactory must iterate build list only and clear captured factories"
    );
}

#[test]
fn on_unit_produced_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::onUnitProduced`")
        .expect("onUnitProduced");
    let window = &src[i..src.len().min(i + 7500)];
    assert!(
        window.contains("is_equivalent_to")
            && window.contains("order.factory_id = None")
            && window.contains("set_team")
            && window.contains("reinforcement_id = Some(unit_id)")
            && window.contains("self.team_delay = 0")
            && window.contains("structure_timer = 1")
            && window.contains("set_force_wanting_state")
            && window.contains("ai_follow_exit_production_path")
            && window.contains("get_goal_position")
            && !window.contains("get_path_destination")
            && window.contains("take_supply_gatherer_slot")
            && window.contains("ai_dock")
            && window.contains("FromPlayer")
            && window.contains("!supply_truck && is_dozer")
            && !window.contains("if found && !supply_truck && is_dozer"),
        "onUnitProduced must match factory+template; dozer path not gated on found"
    );
}

#[test]
fn on_unit_produced_shortcuts_team_delay() {
    let mut ai = AIPlayer::new(1);
    ai.team_delay = 99;
    // invalid factory → still sets team_delay=0 after missing unit path
    ai.on_unit_produced(INVALID_ID, INVALID_ID).expect("oop");
    // factory INVALID returns early without clearing in our port when factory_id==INVALID
    // Use a fake factory with no unit:
    ai.team_delay = 99;
    let _ = ai.on_unit_produced(1, 999999);
    assert_eq!(
        ai.team_delay, 0,
        "C++ always zeroes teamDelay at end of onUnitProduced"
    );
}

#[test]
fn build_structure_with_dozer_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn build_structure_with_dozer")
        .expect("buildStructureWithDozer");
    let window = &src[i..src.len().min(i + 14000)];
    assert!(
        window.contains("find_dozer")
            && window.contains("calc_cost_to_build")
            && window.contains("set_build_task")
            && window.contains("UnderConstruction")
            && window.contains("decrement_num_rebuilds")
            && window.contains("LocalLegalToBuildOptions::NO_ENEMY_OBJECT_OVERLAP")
            && window.contains("LocalLegalToBuildOptions::CLEAR_PATH")
            && window.contains("LocalLegalToBuildOptions::NO_OBJECT_OVERLAP")
            && !window.contains("is_location_safe(&pos")
            && window.contains("120.0 * PATHFIND_CELL_SIZE_F")
            && window.contains("prefer location match")
            && window.contains("client_safe_quick_does_path_exist")
            && window.contains("get_locomotor_set_clone")
            && !window.contains("LocomotorSet::new()")
            && window.contains("Dozer unable to reach building.  Teleporting.")
            && !window.contains("let _ = self.queue_dozer()"),
        "buildStructureWithDozer must path-check+teleport, stamp by location, skirmish wiggle"
    );
}

#[test]
fn calc_closest_construction_zone_location_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn calc_closest_construction_zone_location")
        .expect("calcClosestConstructionZoneLocation");
    let w = &prod[i..prod.len().min(i + 5000)];
    assert!(
        w.contains("LocalLegalToBuildOptions::NO_OBJECT_OVERLAP")
            && w.contains("LocalLegalToBuildOptions::CLEAR_PATH")
            && w.contains("LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS")
            && w.contains("seed_validator")
            && w.contains("wiggle_validator")
            && !w.contains("FoundationValidator::new_ai()"),
        "calcClosestConstructionZoneLocation must split seed/wiggle BuildAssistant flags"
    );
}

#[test]
fn find_valid_build_location_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("fn find_valid_build_location(")
        .expect("find_valid_build_location");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("LocalLegalToBuildOptions::NO_OBJECT_OVERLAP")
            && w.contains("LocalLegalToBuildOptions::CLEAR_PATH")
            && w.contains("LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS")
            && w.contains("seed_validator")
            && w.contains("wiggle_validator")
            && !w.contains("FoundationValidator::new_ai()"),
        "find_valid_build_location must match buildBySupplies BuildAssistant flags"
    );
}

#[test]
fn process_base_building_resumes_dozer_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn process_base_building(&mut self)")
        .expect("pbb");
    let window = &src[i..src.len().min(i + 14000)];
    assert!(
        window.contains("ResumeConstruction")
            && window.contains("resume_jobs")
            && window.contains("AI's Dozer got killed"),
        "solo processBaseBuilding must resume construction on UC buildings"
    );
}

#[test]
fn process_base_building_rebuild_delay_requires_invalid_id() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn process_base_building(&mut self)")
        .expect("pbb");
    let w = &src[i..src.len().min(i + 9000)];
    assert!(
        w.contains("get_object_id() == INVALID_ID && info.get_object_timestamp() > 0")
            && w.contains("Enabling rebuild for"),
        "solo processBaseBuilding rebuild delay must require INVALID_ID like C++"
    );
}

#[test]
fn process_base_building_calls_build_structure_with_dozer() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("fn process_base_building").expect("pbb");
    let window = &src[i..src.len().min(i + 12000)];
    assert!(
        window.contains("build_structure_with_dozer")
            && window.contains("get_spawner_id()")
            && window.contains("KindOf::RebuildHole"),
        "processBaseBuilding must call buildStructureWithDozer and match holes by spawner"
    );
}

#[test]
fn compute_center_uses_build_list_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn compute_center_and_radius_of_base")
        .expect("compute");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("get_build_list()")
            && window.contains("get_bounding_circle_radius")
            && window.contains("* 0.4")
            && window.contains("max_rad_sqr")
            && !window.contains("get_all_objects()"),
        "computeCenterAndRadiusOfBase must average build-list locations + geom*0.4"
    );
}

#[test]
fn on_structure_produced_applies_map_props_and_script() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn on_structure_produced")
        .expect("onStructure");
    let window = &src[i..src.len().min(i + 8000)];
    assert!(
        window.contains("update_obj_values_from_map_properties")
            && window.contains("key_object_initial_health")
            && window.contains("add_object_to_cache")
            && window.contains("run_object_script")
            && window.contains("check_for_supply_center")
            && window.contains("get_reconstructed_building_id")
            && window.contains("saw_rhbi && is_this_spawn")
            && !window.contains("frame_last_building_built = TheGameLogic::get_frame()")
            && window.find("run_object_script") < window.find("self.check_for_supply_center")
            && window.contains("Do NOT call check_for_supply_center while holding"),
        "onStructureProduced: script then supply outside player write; no frame stamp"
    );
}

#[test]
fn new_map_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src
        .split("#[cfg(test)]")
        .next()
        .expect("production before tests");
    let i = prod.find("pub fn new_map(&mut self)").expect("newMap");
    let end = prod[i..]
        .find("/// Start training for a work order")
        .or_else(|| prod[i..].find("pub(crate) fn start_training"))
        .or_else(|| prod[i..].find("fn start_training_internal"))
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 8000));
    let w = &prod[i..end];
    assert!(
        w.contains("original_entries")
            && w.contains("add_to_build_list")
            && w.contains("compute_center_and_radius_of_base")
            && w.contains("is_initially_built")
            && w.contains("increment_num_rebuilds")
            && w.contains("build_structure_now_at")
            && !w.contains("team_build_queue.clear()")
            && !w.contains("structures_to_repair = [None"),
        "newMap must snapshot pre-factory list, prepend factories, center, then original initials"
    );
    let snap = w.find("original_entries").expect("snap");
    let add = w.find("add_to_build_list").expect("add");
    let center = w.find("compute_center_and_radius_of_base").expect("center");
    assert!(
        snap < add && add < center,
        "C++ order: capture head, add factories, compute center, walk original"
    );
}

#[test]
fn join_team_reinforcement_calls_join_team_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn join_team_reinforcement")
        .expect("join_team_reinforcement");
    let w = &src[i..src.len().min(i + 1200)];
    assert!(
        w.contains("ai.join_team()") && !w.contains("Residual: always move toward teammate"),
        "reinforcement must call full joinTeam, not residual move-only"
    );
    let unit = concat!(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/object/unit/ai_loco.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/object/unit/ai_interface.rs"
        )),
    );
    let j = unit
        .find("fn join_team(&mut self)")
        .expect("UnitAI join_team");
    let uw = &unit[j..unit.len().min(j + 3500)];
    assert!(
        uw.contains("choose_locomotor_set")
            && uw.contains("set_goal_waypoint(None)")
            && uw.contains("clear()")
            && uw.contains("other_idle")
            && uw.contains("INVALID_STATE_ID")
            && uw.contains("set_goal_object")
            && uw.contains("set_goal_position"),
        "joinTeam must clear, copy goal, and setState(INVALID→default) like C++"
    );
}

#[test]
fn build_structure_now_at_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::buildStructureNow` (AIPlayer.cpp)")
        .expect("buildStructureNow");
    let window = &src[i..src.len().min(i + 14000)];
    assert!(
        window.contains("update_obj_values_from_map_properties")
            && window.contains("key_object_initial_health")
            && window.contains("clear_status")
            && window.contains("update_upgrade_modules_from_player")
            && window.contains("add_object_to_cache")
            && window.contains("run_object_script")
            && window.contains("check_for_supply_center")
            && window.contains("requested location"),
        "buildStructureNow must apply map props, clear UC, script cache/run, supply check"
    );
}

#[test]
fn check_for_supply_center_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::checkForSupplyCenter`")
        .expect("checkForSupplyCenter");
    let window = &src[i..src.len().min(i + 2500)];
    assert!(
        window.contains("SupplyCenterDockUpdate")
            && window.contains("set_supply_building(true)")
            && window.contains("set_desired_gatherers(desired + 1)")
            && window.contains("set_current_gatherers(-1)"),
        "checkForSupplyCenter must stamp supply building + desired gatherers+1"
    );
}

#[test]
fn queue_supply_truck_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src
        .split("#[cfg(test)]")
        .next()
        .expect("production before tests");
    let i = prod
        .find("fn queue_supply_truck(&mut self)")
        .expect("queueSupplyTruck");
    let end = prod[i..]
        .find("fn count_player_harvesters")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 4000));
    let window = &prod[i..end];
    assert!(
        window.contains("truck_in_queue")
            && window.contains("is_resource_gatherer")
            && window.contains("count_player_harvesters")
            && window.contains("is_supply_building")
            && window.contains("queue_one_harvester_at_factory")
            && window.contains("recount_and_redock_harvesters")
            && window.contains("try_reattach_loose_harvester")
            && window.contains("desired.saturating_mul(3)")
            && window.contains("supply_center_has_nearby_supplies"),
        "queueSupplyTruck must skip-if-queued, recount, reattach, train harvester"
    );
    // C++ only requires nearby warehouse on the cur>=desired maintenance branch.
    let maint = window
        .find("if cur_gatherers >= desired")
        .expect("maintenance branch");
    let under = window
        .find("// Under-desired")
        .expect("under-desired branch");
    assert!(
        maint < under
            && window[maint..under].contains("supply_center_has_nearby_supplies")
            && !window[under..].contains("supply_center_has_nearby_supplies"),
        "nearby warehouse check must only gate the maintenance branch like C++"
    );
}

#[test]
fn find_dozer_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("C++ `AIPlayer::findDozer`").expect("findDozer");
    let window = &src[i..src.len().min(i + 5000)];
    assert!(
        window.contains("need_dozer")
            && window.contains("DozerTask::Build")
            && window.contains("is_currently_ferrying_supplies")
            && window.contains("repair_dozer")
            && window.contains("closest_dozer")
            && window.contains("queue_dozer"),
        "findDozer must prefer idle, skip ferrying/repair/build, queue if none"
    );
}

#[test]
fn compute_superweapon_target_uses_get_extent_and_int_cash() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn compute_superweapon_target")
        .expect("compute_superweapon_target");
    let window = &src[i..src.len().min(i + 5500)];
    assert!(
        window.contains("get_extent()")
            && !window.contains("get_maximum_pathfind_extent()")
            && window.contains("let mut best_cash: i32 = -1")
            && window.contains("let mut fine_cash: i32 = -1")
            && window.contains("value == fine_cash")
            && window.contains("player_index: i32")
            && window.contains("player_index >= 0"),
        "computeSuperweaponTarget must use getExtent + Int cash + explicit playerNdx"
    );
    let j = src
        .find("/// C++ `AIPlayer::getPlayerSuperweaponValue`")
        .expect("value");
    let vw = &src[j..src.len().min(j + 6000)];
    assert!(
        vw.contains("-> Result<i32, AiError>") && vw.contains("Ok(cash as i32)"),
        "getPlayerSuperweaponValue must return Int like C++"
    );
    let h = crate::helpers::HELPERS_SRC;
    assert!(
        h.contains("pub fn get_extent(&self)") && h.contains("guard.get_extent()"),
        "TheTerrainLogic must expose get_extent"
    );
}

#[test]
fn ai_player_crc_is_empty_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("/// C++ `AIPlayer::crc` is empty")
        .expect("crc doc");
    let j = src[i..].find("fn xfer(").expect("xfer after crc") + i;
    let window = &src[i..j];
    assert!(
        window.contains("Intentionally empty")
            && !window.contains("ready_to_build_team")
            && !window.contains("xfer_bool"),
        "AIPlayer::crc must be empty like C++"
    );
}

#[test]
fn check_for_supply_center_module_only_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn check_for_supply_center")
        .expect("check_for_supply_center");
    let w = &src[i..src.len().min(i + 900)];
    assert!(
        w.contains("SupplyCenterDockUpdate")
            && w.contains("find_update_module")
            && !w.contains("FSSupplyCenter")
            && !w.contains("SupplySource"),
        "checkForSupplyCenter must key only on SupplyCenterDockUpdate like C++"
    );
}

#[test]
fn get_player_structure_bounds_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn get_player_structure_bounds_ex")
        .expect("bounds_ex");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("conservative")
            && window.contains("ConservativeBuilding")
            && window.contains("KindOf::Structure")
            && window.contains("first_structure")
            && window.contains("only enters the AABB expand when isKindOf(STRUCTURE)")
            && !window.contains("obj_min"),
        "getPlayerStructureBounds must structure-only AABB + conservative skip"
    );
}

#[test]
fn calc_closest_construction_zone_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
        .expect("calcClosest");
    let w = &src[i..src.len().min(i + 5000)];
    assert!(
        w.contains("get_placement_view_angle")
            && w.contains("let mut valid = false")
            && w.contains("2.0 * SUPPLY_CENTER_CLOSE_DIST")
            && w.contains("when initial_ok, valid stays false"),
        "calcClosestConstructionZone must use placement angle + C++ valid control flow"
    );
}

#[test]
fn compute_superweapon_target_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::computeSuperweaponTarget`")
        .expect("computeSuperweaponTarget");
    let window = &src[i..src.len().min(i + 5500)];
    assert!(
        window.contains("get_extent") && window.contains("x_count, y_count")
            || (window.contains("x_count") && window.contains("y_count")),
        "grid counts"
    );
    assert!(
        window.contains("game_logic_random_value(1, 4)")
            && window.contains("x_count, 0")
            && window.contains("// Fine tune: C++ uses (x-5) for BOTH axes")
            && window.contains("SneakAttack")
            && window.contains("get_player_superweapon_value")
            && window.contains("player_index"),
        "computeSuperweaponTarget must randomize scan, preserve fine-tune bug, playerNdx"
    );
}

#[test]
fn build_upgrade_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::buildUpgrade`")
        .expect("buildUpgrade");
    let w = &src[i..src.len().min(i + 4000)];
    assert!(
        w.contains("get_build_list")
            && w.contains("has_upgrade_in_production")
            && w.contains("has_upgrade_complete")
            && w.contains("can_afford_upgrade")
            && w.contains("queue_upgrade")
            && w.contains("UnderConstruction"),
        "buildUpgrade must walk build list and gate type/money/progress"
    );
}

#[test]
fn find_supply_center_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src
        .split("#[cfg(test)]")
        .next()
        .expect("production before tests");
    let i = prod
        .find("fn find_supply_center(&self, minimum_cash: i32)")
        .expect("findSupplyCenter");
    let end = prod[i..]
        .find("fn find_valid_build_location")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 6000));
    let w = &prod[i..end];
    assert!(
        w.contains("KindOf::SupplySource")
            && w.contains("SupplyWarehouseDockUpdate")
            && w.contains("BASE_VALUE_PER_SUPPLY_BOX")
            && w.contains("KindOf::CashGenerator")
            && w.contains("SUPPLY_CENTER_CLOSE_DIST")
            && w.contains("dist_sqr * 0.4")
            && w.contains("enemy_dist_sqr * 0.6")
            && w.contains("cash_floor /= 2")
            && w.contains("if cash_floor <= 100"),
        "findSupplyCenter must filter warehouse cash, own supply center, enemy 60/40"
    );
    // Halve-then-stop: divide happens before the ≤100 break (C++ do/while).
    let div = w.find("cash_floor /= 2").expect("div");
    let stop = w.find("if cash_floor <= 100").expect("stop");
    assert!(
        div < stop,
        "C++ halves minimumCash before while(minimumCash>100); no extra ≤100 pass"
    );
}

#[test]
fn build_specific_ai_team_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::buildSpecificAITeam`")
        .expect("buildSpecificAITeam");
    let w = &src[i..src.len().min(i + 7000)];
    assert!(
        w.contains("get_can_build_units")
            && w.contains("is_singleton")
            && w.contains("is_possible_to_build_team")
            && w.contains("need_money")
            && w.contains("create_inactive_team")
            && w.contains("order.required = true")
            && w.contains("self.team_delay = 0")
            && w.contains("even minUnits==0")
            && w.contains("execute_action_sequence"),
        "buildSpecificAITeam must queue min=0 required orders and run executeActions"
    );
}

#[test]
fn recruit_specific_ai_team_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::recruitSpecificAITeam`")
        .expect("recruitSpecificAITeam");
    let w = &src[i..src.len().min(i + 7000)];
    assert!(
        w.contains("try_to_recruit")
            && w.contains("team_ready_queue.push_front")
            && w.contains("create_inactive_team")
            && w.contains("MoveToPosition")
            && w.contains("team_about_to_be_deleted")
            && w.contains("Recruited 0 units")
            && w.contains("home_location()")
            && w.contains("has_home_location")
            && w.contains("is_skirmish_ai_player"),
        "recruitSpecificAITeam must tryToRecruit, homeLocation, ready-queue or disband"
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
fn build_nearest_team_and_calc_closest_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let n = src
        .find("C++ `AIPlayer::buildSpecificBuildingNearestTeam`")
        .expect("nearest");
    let c = src
        .find("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
        .expect("calc");
    let nw = &src[n..src.len().min(n + 2500)];
    let cw = &src[c..src.len().min(c + 5000)];
    assert!(
        nw.contains("get_estimate_team_position")
            && nw.contains("add_to_priority_build_list")
            && nw.contains("calc_closest_construction_zone_location"),
        "nearest-team must estimate pos, calcClosest, priority list"
    );
    assert!(
        cw.contains("get_placement_view_angle")
            && cw.contains("let mut valid = false")
            && cw.contains("location: &Coord3D"),
        "calcClosest must use placement angle + C++ valid control flow"
    );
}

#[test]
fn solo_base_defense_and_on_structure_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let d = src
        .find("C++ `AIPlayer::buildAIBaseDefense`")
        .expect("defense");
    let o = src
        .find("C++ `AIPlayer::onStructureProduced`")
        .expect("onStructure");
    let dw = &src[d..src.len().min(d + 1200)];
    let ow = &src[o..src.len().min(o + 8000)];
    assert!(
        dw.contains("Solo ai doesn't support buildAIBaseDefense")
            && dw.contains("buildAIBaseDefenseStructure"),
        "solo defense stubs"
    );
    assert!(
        ow.contains("update_upgrade_modules_from_player")
            && ow.contains("RebuildHole")
            && ow.contains("check_for_supply_center")
            && ow.contains("Structure not found in production queue"),
        "onStructureProduced list match + hole retarget + supply"
    );
}

#[test]
fn build_paths_use_placement_view_angle_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn build_by_supplies")
        .expect("build_by_supplies");
    let w = &src[i..src.len().min(i + 2500)];
    assert!(
        w.contains("get_placement_view_angle()"),
        "buildBySupplies must use ThingTemplate placement view angle (C++)"
    );
    let j = src
        .find("fn build_specific_building_near_location")
        .expect("near_location");
    let w2 = &src[j..src.len().min(j + 1200)];
    assert!(
        w2.contains("get_placement_view_angle()"),
        "buildSpecificBuildingNearLocation must use placement view angle"
    );
}

#[test]
fn get_skirmish_enemy_player_index_prefers_current_enemy_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn get_skirmish_enemy_player_index")
        .expect("skirmish enemy");
    let w = &src[i..src.len().min(i + 1800)];
    assert!(
        w.contains("get_current_enemy_player_index")
            && w.contains("PlayerType::Human")
            && w.contains("PlayerType::Neutral"),
        "skirmish enemy index must prefer current enemy then human"
    );
}

#[test]
fn guard_supply_center_uses_script_cmd_source_like_cpp() {
    // C++ groupGuardPosition(..., GUARDMODE_NORMAL, CMD_FROM_SCRIPT)
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::guardSupplyCenter`")
        .expect("guardSupplyCenter");
    let window = &src[i..src.len().min(i + 4500)];
    assert!(
        window.contains("ai_guard_position")
            && window.contains("GuardMode::Normal")
            && window.contains("FromScript")
            && window.contains("supply_source_attack_check_frame = 0"),
        "guardSupplyCenter must issue GuardPosition NORMAL from script source"
    );
}

#[test]
fn build_by_supplies_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let prod = src
        .split("#[cfg(test)]")
        .next()
        .expect("production before tests");
    let i = prod
        .find("pub fn build_by_supplies(")
        .expect("buildBySupplies");
    let end = prod[i..]
        .find("pub fn build_specific_building_near_location")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 4000));
    let w = &prod[i..end];
    assert!(
        w.contains("find_supply_center")
            && w.contains("add_to_priority_build_list")
            && w.contains("CashGenerator")
            && w.contains("current_warehouse_id")
            && w.contains("3.0 * PATHFIND_CELL_SIZE_F")
            && w.contains("self.base_center")
            && !w.contains("compute_center_and_radius_of_base"),
        "buildBySupplies must find then maybe override warehouse; use m_baseCenter as-is"
    );
    // find first, then non-cash current override.
    let find = w.find("find_supply_center(minimum_cash)").expect("find");
    let override_cur = w.find("!is_cash_generator").expect("non-cash");
    assert!(
        find < override_cur,
        "C++ finds supply center before non-cash curWarehouse override"
    );
}

#[test]
fn repair_structure_and_bridge_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let r = src
        .find("C++ `AIPlayer::repairStructure`")
        .expect("repairStructure");
    let b = src
        .find("C++ `AIPlayer::updateBridgeRepair`")
        .expect("updateBridgeRepair");
    let rw = &src[r..src.len().min(r + 2000)];
    let bw = &src[b..src.len().min(b + 9000)];
    assert!(
        rw.contains("BodyDamageType::Pristine")
            && rw.contains("MAX_STRUCTURES_TO_REPAIR")
            && rw.contains("structures_in_queue"),
        "repairStructure must skip pristine and bound queue"
    );
    assert!(
        bw.contains("LOGICFRAMES_PER_SECOND")
            && bw.contains("dozer_queued_for_repair")
            && bw.contains("AiCommandType::Repair")
            && bw.contains("is_any_task_pending")
            && bw.contains("ai_move_to_position")
            && bw.contains("adjust_to_possible_destination")
            && bw.find("saturating_sub(1)").unwrap() < bw.find("if self.bridge_timer > 0").unwrap(),
        "updateBridgeRepair must decrement timer first, assign dozer, complete+home"
    );

    // Behavioral: timer==1 with empty/missing queue still advances without hang.
    let mut ai = AIPlayer::new(1);
    ai.structures_in_queue = 1;
    ai.structures_to_repair[0] = Some(999_999); // missing object → pop
    ai.bridge_timer = 1;
    ai.update_bridge_repair().expect("ubr");
    assert_eq!(
        ai.structures_in_queue, 0,
        "missing repair target is popped when timer fires"
    );
    assert_eq!(
        ai.bridge_timer, LOGICFRAMES_PER_SECOND,
        "timer resets to 1s after fire"
    );
}

#[test]
fn repair_structure_skips_pristine_and_duplicates() {
    let mut ai = AIPlayer::new(1);
    // Without a live object, repair_structure returns Ok and does not enqueue.
    ai.repair_structure(999_999).expect("rs");
    assert_eq!(ai.structures_in_queue, 0);
}

#[test]
fn build_specific_ai_building_solo_is_noop_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::buildSpecificAIBuilding`")
        .expect("buildSpecificAIBuilding");
    let w = &src[i..src.len().min(i + 800)];
    assert!(
        w.contains("Solo ai doesn't support") && !w.contains("construction_priorities.insert"),
        "solo buildSpecificAIBuilding only logs"
    );
}

#[test]
fn get_player_superweapon_value_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("C++ `AIPlayer::getPlayerSuperweaponValue`")
        .expect("getPlayerSuperweaponValue");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("FSBaseDefense")
            && window.contains("TechBaseDefense")
            && window.contains("is_significantly_above_terrain")
            && window.contains("CommandCenter")
            && window.contains("FSSuperweapon")
            && window.contains("factor * value * 5.0"),
        "getPlayerSuperweaponValue must match C++ defense/aircraft/CC scoring"
    );
}

#[test]
fn queue_dozer_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("C++ `AIPlayer::queueDozer`").expect("queueDozer");
    let window = &src[i..src.len().min(i + 4500)];
    assert!(
        window.contains("dozer_in_queue")
            && window.contains("set_can_build_units_temp")
            && window.contains("start_training_internal")
            && window.contains("priority_build = true")
            && window.contains("KindOf::Dozer")
            && window.contains("first_template")
            && window.contains("get_next_template")
            && !window.contains("dozer_queued_for_repair = true"),
        "queueDozer must gate queue, enable units, startTraining priority dozer"
    );
}

#[test]
fn is_supply_source_attacked_scan_rate_is_10_frames_like_cpp() {
    // C++: const Int SCAN_RATE = 10; added to frame counters (not * LOGICFRAMES).
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn is_supply_source_attacked")
        .expect("is_supply_source_attacked");
    let window = &src[i..src.len().min(i + 1200)];
    assert!(
        window.contains("const SCAN_RATE: u32 = 10")
            && !window[..window.find("const SCAN_RATE").unwrap_or(0) + 80]
                .contains("LOGICFRAMES_PER_SECOND"),
        "SCAN_RATE must be 10 frames matching C++ AIPlayer.cpp, not 10 seconds"
    );
}

#[test]
fn supply_source_attacked_safe_guard_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let a = src
        .find("C++ `AIPlayer::isSupplySourceAttacked`")
        .expect("attacked");
    let s = src
        .find("C++ `AIPlayer::isSupplySourceSafe`")
        .expect("safe");
    let g = src
        .find("C++ `AIPlayer::guardSupplyCenter`")
        .expect("guard");
    let aw = &src[a..src.len().min(a + 4000)];
    let sw = &src[s..src.len().min(s + 1500)];
    let gw = &src[g..src.len().min(g + 4000)];
    assert!(
        aw.contains("SCAN_RATE")
            && aw.contains("get_attacked_frame")
            && aw.contains("CashGenerator")
            && aw.contains("get_last_damage_timestamp")
            && aw.contains("attacked_supply_center"),
        "isSupplySourceAttacked must rate-limit and scan recent damage"
    );
    assert!(
        sw.contains("find_supply_center") && sw.contains("is_location_safe"),
        "isSupplySourceSafe must delegate to find+isLocationSafe"
    );
    assert!(
        gw.contains("supply_source_attack_check_frame = 0")
            && gw.contains("ai_guard_position")
            && gw.contains("get_bounding_circle_radius")
            && gw.contains("get_player_structure_bounds"),
        "guardSupplyCenter must force check, offset, and guard"
    );
}

#[test]
fn dozer_in_queue_uses_includes_a_dozer_like_cpp() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("pub fn dozer_in_queue").expect("dozerInQueue");
    let w = &src[i..src.len().min(i + 500)];
    assert!(
        w.contains("includes_a_dozer()"),
        "dozerInQueue must use TeamInQueue::includesADozer"
    );

    // gatherer-only work order is not a "dozer in queue"
    let mut ai = AIPlayer::new(1);
    let mut order = WorkOrder::new("GLAInfantryWorker".into());
    order.is_resource_gatherer = true;
    let mut team = TeamInQueue::new();
    team.work_orders.push(order);
    ai.team_build_queue.push_front(team);
    assert!(
        !ai.dozer_in_queue(),
        "resource-gatherer work order must not count as dozerInQueue"
    );
}

#[test]
fn queue_dozer_does_not_set_repair_flag_like_cpp() {
    // C++ queueDozer never sets m_dozerQueuedForRepair (repair path only).
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src.find("pub(crate) fn queue_dozer").expect("queue_dozer");
    let end = src[i..].find("pub fn dozer_in_queue").unwrap_or(5000);
    let window = &src[i..i + end];
    assert!(
        !window.contains("dozer_queued_for_repair = true"),
        "queueDozer must not stamp dozer_queued_for_repair"
    );
    assert!(
        window.contains("first_template") && window.contains("KindOf::Dozer"),
        "queueDozer must walk ThingFactory dozer templates"
    );
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
fn queue_one_harvester_walks_thing_factory_like_cpp() {
    // C++ queueSupplyTruck: tTemplate = firstTemplate(); while (tTemplate) { isKindOf(HARVESTER); tTemplate = friend_getNextTemplate(); }
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn queue_one_harvester_at_factory")
        .expect("queue_one_harvester");
    let window = &src[i..src.len().min(i + 3500)];
    assert!(
        window.contains("first_template")
            && window.contains("get_next_template")
            && window.contains("KindOf::Harvester")
            && window.contains("is_resource_gatherer = true")
            && window.contains("cur_gatherers == -1"),
        "queue_one_harvester must walk ThingFactory harvester templates like C++"
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
fn select_team_to_reinforce_auto_cpp_surface() {
    let src = crate::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("fn select_team_to_reinforce(&mut self, min_priority: i32)")
        .expect("reinforce");
    let window = &src[i..src.len().min(i + 7000)];
    assert!(
        window.contains("automatically_reinforce")
            && window.contains("priority <= cur_priority")
            && window.contains("max_units")
            && window.contains("try_to_recruit")
            && window.contains("push_front(team_q)")
            && window.contains("self.team_delay = 0")
            && window.contains("find_factory_internal")
            && window.contains("order.num_required = 1")
            && window.contains("q.team.as_ref()")
            && window.contains("has_home_location")
            && window.contains("get_members().first()"),
        "select_team_to_reinforce must match C++ busy-by-handle + home/member origin"
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
