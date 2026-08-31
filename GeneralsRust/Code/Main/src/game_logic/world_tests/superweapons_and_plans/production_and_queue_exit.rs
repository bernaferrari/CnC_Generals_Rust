//! Behavior suite extracted from `superweapons_and_plans`.
use super::*;

#[test]
fn script_skirmish_attack_nearest_group_uses_relationship_and_cell_value() {
    use crate::game_logic::AIState;
    use crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL;
    use gamelogic::common::Relationship;
    use gamelogic::scripting::request_host_skirmish_attack_nearest_group;

    if let Ok(mut pm) = gamelogic::object::collide::partition_manager::PARTITION_MANAGER.write() {
        pm.clear();
    }
    let _ = gamelogic::scripting::take_host_skirmish_attack_nearest_group_requests();

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;

    let mut p0 = Player::new(0, Team::USA, "USA1", true);
    let mut p1 = Player::new(1, Team::USA, "USA2", false);
    let p_neutral = Player::new(3, Team::Neutral, "Neutral", false);
    p0.set_map_relationship(1, Relationship::Enemies);
    p1.set_map_relationship(0, Relationship::Enemies);
    p0.resources.supplies = 100_000;
    p1.resources.supplies = 100_000;
    logic.add_player(p0);
    logic.add_player(p1);
    logic.add_player(p_neutral);

    ensure_test_tank_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let mut cheap = ThingTemplate::new("CheapHut");
    cheap
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    cheap.build_cost.supplies = 30;
    logic.templates.insert("CheapHut".into(), cheap);

    let mut decoy = ThingTemplate::new("NeutralPalace");
    decoy
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    decoy.build_cost.supplies = 5000;
    logic.templates.insert("NeutralPalace".into(), decoy);

    let attacker = logic
        .create_object("TestTank", Team::USA, glam::Vec3::ZERO)
        .expect("attacker");
    if let Some(obj) = logic.host_object_mut(attacker) {
        obj.team_instance_name = "teamAmerica".into();
        obj.owner_player_id = Some(0);
        obj.shroud_clearing_range = 0.0;
    }

    // Same-faction USA enemy pair in cell (5, 0). Each costs 30; aggregate 60 > 50.
    // Off-corner so dest must be the cell corner, not an object pose.
    let e1 = logic
        .create_object("CheapHut", Team::USA, glam::Vec3::new(205.0, 0.0, 10.0))
        .expect("e1");
    let e2 = logic
        .create_object("CheapHut", Team::USA, glam::Vec3::new(215.0, 0.0, 15.0))
        .expect("e2");
    for id in [e1, e2] {
        if let Some(obj) = logic.host_object_mut(id) {
            obj.owner_player_id = Some(1);
            obj.partition_cash_value = 30;
            obj.shroud_clearing_range = 0.0;
        }
    }

    // Closer Neutral 5000-cost must be ignored (ALLOW_ENEMIES only). Placed
    // far beyond the enemy cells so a misattributed leftover-partition entry
    // still loses the breadth-first search to the true enemy cell.
    let n = logic
        .create_object(
            "NeutralPalace",
            Team::Neutral,
            glam::Vec3::new(801.0, 0.0, 0.0),
        )
        .expect("neutral");
    if let Some(obj) = logic.host_object_mut(n) {
        obj.owner_player_id = Some(3);
        obj.partition_cash_value = 5000;
        obj.shroud_clearing_range = 0.0;
    }
    // The leftover partition bridge is a process-global other tests may
    // repopulate; serialize and clear immediately before the scripted frame
    // so the host fallback BFS decides the destination.
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Ok(mut pm) = gamelogic::object::collide::partition_manager::PARTITION_MANAGER.write() {
        pm.clear();
    }
    request_host_skirmish_attack_nearest_group("teamAmerica", 4, 50);
    logic.evaluate_and_execute_scripts(0.0);

    let after = logic.host_object(attacker).expect("attacker");
    let dest = after.requested_destination.expect("attack-move dest");
    let cell = PARTITION_CELL_SIZE_RESIDUAL;
    assert!(
        (dest.x - 5.0 * cell).abs() < 0.1 && (dest.z - 0.0).abs() < 0.1,
        "must attack-move to enemy cell corner, not Neutral/object/origin, dest={dest:?}"
    );
    assert_ne!(
        dest,
        glam::Vec3::new(801.0, 0.0, 0.0),
        "Neutral high-cost must not win ALLOW_ENEMIES"
    );
    assert!(
        after.ai_state == AIState::AttackMoving || after.ai_state == AIState::Moving,
        "team must attack-move, state={:?}",
        after.ai_state
    );
}

#[test]
fn script_named_set_topple_direction_used_by_live_topple() {
    use crate::game_logic::host_structure_topple::is_structure_topple_candidate;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    assert!(is_structure_topple_candidate("TestBarracks", true));
    let id = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("bldg");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "FallingHall".into();
    }
    gamelogic::scripting::request_host_script_topple_direction("FallingHall", 0.0, 1.0);
    let started = logic
        .host_object_mut(id)
        .expect("bldg")
        .begin_structure_topple(0, Some((100.0, 0.0)));
    assert!(started, "named structure must begin topple");
    let st = logic
        .host_object(id)
        .and_then(|o| o.structure_topple_data.clone())
        .expect("topple data");
    assert!(
        (st.dir_x - 0.0).abs() < 0.05 && (st.dir_y - 1.0).abs() < 0.05,
        "NAMED_SET_TOPPLE_DIRECTION must replace attacker-away dir, got ({}, {})",
        st.dir_x,
        st.dir_y
    );
}

#[test]
fn enqueue_production_full_queue_does_not_charge_resources() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    }

    let charged_supplies = game_logic
        .get_player(0)
        .expect("player should exist")
        .effective_supplies();
    assert_eq!(
        charged_supplies,
        100_000 - (DEFAULT_PRODUCTION_QUEUE_LIMIT as u32 * 100)
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        DEFAULT_PRODUCTION_QUEUE_LIMIT
    );

    assert!(!game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));

    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .effective_supplies(),
        charged_supplies,
        "full production queues must not charge resources"
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        DEFAULT_PRODUCTION_QUEUE_LIMIT,
        "full production queues should not accept an extra item"
    );
}

#[test]
fn enqueue_production_requires_player_money_state() {
    let mut game_logic = GameLogic::new();
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    assert!(!game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        0,
        "production should not queue for free without player state"
    );
}

#[test]
fn enqueue_infantry_on_command_center_fails_barracks_succeeds() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_command_center_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let cc_id = game_logic
        .create_object("TestCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    assert!(
        !game_logic.enqueue_production(cc_id, "TestInfantry".to_string()),
        "Command Center must not produce infantry (the train_fail_enqueue weasel)"
    );

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("barracks");
    assert!(
        game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()),
        "completed barracks with money must enqueue infantry"
    );
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|b| b.building_data.as_ref())
            .map(|b| b.production_queue.len())
            .unwrap_or(0),
        1
    );
}

#[test]
fn host_construction_completes_without_coupled_shadow() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    if let Some(t) = game_logic.templates.get_mut("TestBarracks") {
        t.build_time = 1.0;
    }
    let id = game_logic
        .create_object_under_construction("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("uc barracks");
    // C++ DozerAIUpdate.cpp:511-517 — only a dozer docked at the ACTION dock
    // advances construction percent; author the retail builder fixture.
    ensure_test_dozer_template(&mut game_logic);
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("builder dozer");
    {
        let barracks = game_logic.host_object_mut(id).expect("barracks");
        barracks.builder_id = Some(dozer_id);
    }
    {
        let dozer = game_logic.host_object_mut(dozer_id).expect("dozer");
        dozer.set_target(Some(id));
        dozer.set_ai_state(AIState::Constructing);
        dozer.status.moving = false;
        dozer.dozer_dock_action = Some(dozer.get_position());
    }
    assert!(
        game_logic
            .host_object(id)
            .is_some_and(|o| o.status.under_construction)
    );
    for _ in 0..80 {
        game_logic.update_with_dt(1.0 / 30.0);
    }
    let obj = game_logic.host_object(id).expect("still exists");
    assert!(
        !obj.status.under_construction,
        "host-only construction must finish (percent={})",
        obj.construction_percent
    );
    assert!(
        game_logic.enqueue_production(id, "TestInfantry".to_string()),
        "completed host barracks must accept infantry enqueue"
    );
}

#[test]
fn host_construction_completes_when_sole_tick_unmapped() {
    use crate::game_logic::AIState;
    // Coupled sole-tick with no live shadow map: host must store percent and
    // complete. The previous hole computed `projected` then discarded it
    // (`if !sole` never assigned), so barracks stayed UC forever.
    // Production keeps GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY default-off
    // (host GameLogic is the sole writer, C++ single-store); the sole-tick
    // contract under test opts in via the retail env channel.
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_construction =
        std::env::var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    crate::gameworld_shadow::begin_shadow_coupled_tick();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(crate::gameworld_shadow::gameworld_construction_sole_tick_enabled());
        let mut game_logic = GameLogic::new();
        ensure_test_player_for_team(&mut game_logic, Team::USA);
        ensure_test_barracks_template(&mut game_logic);
        ensure_test_infantry_template(&mut game_logic);
        if let Some(t) = game_logic.templates.get_mut("TestBarracks") {
            t.build_time = 1.0;
        }
        let id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 40.0))
            .expect("uc barracks");
        // C++ DozerAIUpdate.cpp:511-517 — even the unmapped sole-tick fail-open
        // path is dozer-driven; author the retail builder fixture.
        ensure_test_dozer_template(&mut game_logic);
        let dozer_id = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(40.0, 0.0, 40.0))
            .expect("builder dozer");
        {
            let barracks = game_logic.host_object_mut(id).expect("barracks");
            barracks.builder_id = Some(dozer_id);
        }
        {
            let dozer = game_logic.host_object_mut(dozer_id).expect("dozer");
            dozer.set_target(Some(id));
            dozer.set_ai_state(AIState::Constructing);
            dozer.status.moving = false;
            dozer.dozer_dock_action = Some(dozer.get_position());
        }
        assert!(
            !crate::gameworld_shadow::coupled_host_mapped(id),
            "this test is the unmapped fail-open path"
        );
        for _ in 0..80 {
            game_logic.update_with_dt(1.0 / 30.0);
        }
        let obj = game_logic.host_object(id).expect("still exists");
        assert!(
            !obj.status.under_construction,
            "unmapped sole-tick construction must finish (percent={})",
            obj.construction_percent
        );
        assert!(
            game_logic.enqueue_production(id, "TestInfantry".to_string()),
            "completed unmapped barracks must accept infantry enqueue"
        );
    }));
    crate::gameworld_shadow::end_shadow_coupled_tick();
    match prev_construction {
        Some(v) => {
            crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", v)
        }
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY"),
    }
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    result.expect("unmapped sole-tick construction test");
}

#[test]
fn cancel_production_requires_player_money_state_for_refund() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");
    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    game_logic.players.clear();

    assert!(!game_logic.cancel_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .host_object(barracks_id)
            .and_then(|building| building.building_data.as_ref())
            .expect("barracks should have building data")
            .production_queue
            .len(),
        1,
        "cancelling without player state must not drop queued production"
    );
}

#[test]
fn destroying_producer_refunds_queued_production_to_owner() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist")
            .effective_supplies(),
        99_900,
        "queued production should charge the owner before destruction"
    );

    game_logic.mark_object_for_destruction(barracks_id, Some(Team::GLA));
    // C++ cancelAndRefund fires at death start (before topple/collapse deferral).
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist")
            .effective_supplies(),
        100_000,
        "producer death should refund queued production to the owner"
    );
    assert_eq!(
        game_logic
            .get_player(2)
            .expect("GLA player should exist")
            .effective_supplies(),
        100_000,
        "killer should not receive the destroyed producer's queue refund"
    );
    // StructureTopple/Collapse may defer remove across frames.
    let mut removed = false;
    for _ in 0..600 {
        game_logic.update();
        if game_logic.host_object(barracks_id).is_none() {
            removed = true;
            break;
        }
    }
    assert!(
        removed,
        "destroyed producer should be removed after topple/collapse residual"
    );
}

#[test]
fn attack_ground_damages_enemy_near_impact_point() {
    let mut game_logic = GameLogic::new();
    let attacker_id = setup_ground_attacker(
        &mut game_logic,
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(15.0, 0.0, 0.0),
    );
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("target should be created from template");

    game_logic.frame = 60; // t=1s, enough for first shot with reload_time 0.25
    let health_before = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let health_after = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;
    assert!(
        health_after < health_before,
        "ground attack should damage units near impact point"
    );
}

#[test]
fn search_and_destroy_active_queues_and_stops_idle_loop() {
    use crate::game_logic::host_strategy_center::{
        BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO, HostBattlePlan,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::SearchAndDestroy, Some(sc_id),));
    game_logic.queued_audio_events.clear();
    advance_battle_plan_door_to_active(&mut game_logic);
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO
                && e.object_id == Some(sc_id)
                && e.is_looping
                && !e.stop
        }),
        "ACTIVE SearchAndDestroy must start idle loop: {:?}",
        game_logic.queued_audio_events
    );

    game_logic.queued_audio_events.clear();
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == BATTLE_PLAN_SEARCH_AND_DESTROY_IDLE_AUDIO
                && e.object_id == Some(sc_id)
                && e.stop
        }),
        "leaving SearchAndDestroy must stop idle loop: {:?}",
        game_logic.queued_audio_events
    );
}

#[test]
fn scud_storm_door_open_queues_idle_loop() {
    let mut game_logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAScudStorm");
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(2000.0);
    game_logic.templates.insert("GLAScudStorm".to_string(), t);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let id = game_logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::ZERO)
        .expect("scud");
    game_logic.frame = 0;
    if let Some(o) = game_logic.host_object_mut(id) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    game_logic.update_ai(&[id], 1.0 / 30.0);
    {
        let o = game_logic.objects.get_mut(&id).expect("scud after tick");
        let data = o
            .missile_launcher_building
            .as_mut()
            .expect("missile launcher door SM");
        data.pending_idle_audio = Some("ScudStormIdleLoop".to_string());
        data.stop_idle_audio = false;
    }
    game_logic.queued_audio_events.clear();
    game_logic.frame = 1;
    game_logic.update_ai(&[id], 1.0 / 30.0);
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == "ScudStormIdleLoop"
                && e.object_id == Some(id)
                && e.is_looping
                && !e.stop
        }),
        "DOOR_OPEN must queue ScudStormIdleLoop: {:?}",
        game_logic.queued_audio_events
    );
}
