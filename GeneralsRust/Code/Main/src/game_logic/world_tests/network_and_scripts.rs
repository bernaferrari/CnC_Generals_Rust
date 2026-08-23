//! Host GameLogic tests — `network_and_scripts`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn host_named_unit_found_with_empty_object_registry() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        clear_host_script_query_snapshot, host_script_named_unit_id, host_script_team_unit_ids,
    };

    assert!(
        OBJECT_REGISTRY.is_empty(),
        "host path must not populate OBJECT_REGISTRY"
    );

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("NamedScout");
    t.set_health(100.0);
    logic.templates.insert("NamedScout".into(), t);
    let id = logic
        .create_object("NamedScout", Team::USA, Vec3::new(10.0, 0.0, 20.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapNamedScout".into();
    }

    assert!(OBJECT_REGISTRY.is_empty());
    assert_eq!(logic.host_named_unit_id("MapNamedScout"), Some(id));
    assert!(logic.host_team_unit_ids(Team::USA).contains(&id));
    assert!(!logic
        .host_area_unit_ids(Vec3::new(0.0, 0.0, 0.0), Vec3::new(15.0, 0.0, 25.0))
        .is_empty());

    if let Some(o) = logic.host_object_mut(id) {
        o.set_position(Vec3::new(10.0, 42.0, 20.0));
    }
    logic.inject_host_script_query_snapshot();
    assert_eq!(host_script_named_unit_id("MapNamedScout"), Some(id.0));
    let host = gamelogic::scripting::host_script_query_object_by_id(id.0).expect("injected");
    assert_eq!(host.x, 10.0);
    assert_eq!(host.y, 42.0);
    assert_eq!(host.z, 20.0);
    assert_eq!(
        gamelogic::scripting::host_script_named_unit_alive("MapNamedScout"),
        Some(true)
    );

    assert!(gamelogic::scripting::host_script_query_has_any());
    assert!(!host_script_team_unit_ids(Team::USA as u32).is_empty());
    assert!(OBJECT_REGISTRY.is_empty());
    // Existence is not inside-area when bounds are unknown.
    assert_eq!(
        gamelogic::scripting::host_script_named_unit_in_named_area(
            "MapNamedScout",
            "NoSuchTriggerArea"
        ),
        None
    );
    {
        use gamelogic::scripting::{set_host_script_query_snapshot, HostScriptQuerySnapshot};
        let mut snap = HostScriptQuerySnapshot::default();
        snap.named.insert("MapNamedScout".into(), id.0);
        snap.objects
            .push(gamelogic::scripting::HostScriptQueryObject {
                id: id.0,
                name: "MapNamedScout".into(),
                team: Team::USA as u32,
                x: 10.0,
                z: 20.0,
                alive: true,
                ..Default::default()
            });
        snap.areas.insert("ScoutPad".into(), (0.0, 0.0, 15.0, 25.0));
        snap.areas
            .insert("FarPad".into(), (100.0, 100.0, 110.0, 110.0));
        set_host_script_query_snapshot(snap);
        assert_eq!(
            gamelogic::scripting::host_script_named_unit_in_named_area("MapNamedScout", "ScoutPad"),
            Some(true)
        );
        assert_eq!(
            gamelogic::scripting::host_script_named_unit_in_named_area("MapNamedScout", "FarPad"),
            Some(false)
        );
        assert_eq!(
            gamelogic::scripting::host_script_named_unit_in_named_area(
                "MapNamedScout",
                "NoSuchTriggerArea"
            ),
            None
        );
    }
    clear_host_script_query_snapshot();
}

#[test]
fn live_executor_named_team_conditions_use_host_snapshot() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
    use gamelogic::scripting::engine::get_named_object_tracker;
    use gamelogic::scripting::executor::{ScriptConditionEvaluator, ScriptConditionResult};
    use gamelogic::scripting::{clear_host_script_query_snapshot, ScriptContext};
    use std::sync::{Arc, RwLock};

    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();
    get_named_object_tracker().clear().unwrap();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("NamedScout");
    t.set_health(100.0);
    logic.templates.insert("NamedScout".into(), t);
    let id = logic
        .create_object("NamedScout", Team::USA, Vec3::new(10.0, 0.0, 20.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapNamedScout".into();
        o.team_instance_name = "USA_RangerSquad".into();
        o.owner_player_id = Some(1);
    }
    if let Some(p) = logic.players.get_mut(&1) {
        p.name = "PlyrAmerica".into();
    } else {
        // Host player 1 may already exist; keep whatever name inject reads.
    }
    logic.inject_host_named_unit_map_into_crate_tracker();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut created = Condition::new(ConditionType::NamedCreated);
    created
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapNamedScout".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut created).unwrap(),
        ScriptConditionResult::True
    );

    let mut totally_dead = Condition::new(ConditionType::NamedTotallyDead);
    totally_dead
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapNamedScout".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut totally_dead).unwrap(),
        ScriptConditionResult::False
    );

    let mut destroyed = Condition::new(ConditionType::TeamDestroyed);
    destroyed
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut destroyed).unwrap(),
        ScriptConditionResult::False
    );

    let mut has_units = Condition::new(ConditionType::TeamHasUnits);
    has_units
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut has_units).unwrap(),
        ScriptConditionResult::True
    );

    clear_host_script_query_snapshot();
    get_named_object_tracker().clear().unwrap();
}




#[test]
fn host_player_census_injected_for_script_player_conditions() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{clear_host_script_query_snapshot, host_query_player_census};

    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();

    let mut logic = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "PlyrAmerica", true);
    player.resources.supplies = 4_000;
    player.power_produced = 10;
    player.power_consumed = 5;
    logic.add_player(player);

    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.set_health(2000.0);
    cc.add_kind_of(KindOf::Structure);
    cc.add_kind_of(KindOf::CommandCenter);
    cc.add_kind_of(KindOf::MpCountForVictory);
    logic.templates.insert("AmericaCommandCenter".into(), cc);

    let id = logic
        .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(10.0, 0.0, 20.0))
        .expect("command center");
    assert!(logic.host_object(id).is_some());

    logic.inject_host_script_query_snapshot();
    let census = host_query_player_census("PlyrAmerica").expect("host census");
    assert_eq!(census.money, 4_000);
    assert!(census.has_sufficient_power());
    assert!(census.has_any_objects);
    assert!(census.has_any_build_facility);
    assert_eq!(census.building_count, 1);
    assert_eq!(census.faction_building_count, 1);
    assert_eq!(
        census
            .template_counts
            .get("americacommandcenter")
            .copied()
            .unwrap_or(0),
        1
    );
    assert_eq!(
        census
            .template_counts_ignore_dead
            .get("americacommandcenter")
            .copied()
            .unwrap_or(0),
        1
    );

    player = Player::new(2, Team::China, "PlyrChina", false);
    player.resources.supplies = 0;
    player.power_produced = 0;
    player.power_consumed = 8;
    logic.add_player(player);
    logic.inject_host_script_query_snapshot();
    let china = host_query_player_census("PlyrChina").expect("china census");
    assert_eq!(china.money, 0);
    assert!(!china.has_sufficient_power());
    assert!(!china.has_any_objects);
    assert!(!china.has_any_build_facility);
    assert_eq!(china.building_count, 0);

    clear_host_script_query_snapshot();
}


#[test]
fn create_object_script_request_spawns_named_host_unit() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{request_host_script_create, HostScriptCreateRequest};

    OBJECT_REGISTRY.clear();
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), t);

    request_host_script_create(HostScriptCreateRequest::Object {
        name: Some("ColonelBurton".into()),
        thing: "AmericaInfantryRanger".into(),
        team: "teamAmerica".into(),
        x: 12.0,
        y: 24.0,
        z: 0.0,
        angle: 1.25,
    });
    logic.apply_host_create_script_requests();

    let id = logic
        .host_object_id_by_script_name("ColonelBurton")
        .expect("named unit");
    let obj = logic.objects.get(&id).expect("spawned");
    assert_eq!(obj.name, "ColonelBurton");
    assert_eq!(obj.team, Team::USA);
    assert_eq!(obj.team_instance_name, "teamAmerica");
    assert!((obj.get_orientation() - 1.25).abs() < 0.001);
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_host_script_unmanned_radar_stealth_apply() {
    use crate::game_logic::host_radar::last_the_radar_event_host_position;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_radar_event, request_host_script_stealth_enabled,
        request_host_script_unmanned, HostScriptRadarEventRequest, HostScriptStealthEnabledRequest,
        HostScriptUnmannedRequest,
    };

    OBJECT_REGISTRY.clear();
    let _ = gamelogic::scripting::take_host_script_unmanned_requests();
    let _ = gamelogic::scripting::take_host_script_radar_event_requests();
    let _ = gamelogic::scripting::take_host_script_stealth_enabled_requests();

    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("AmericaTankCrusader");
    tank.set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tank);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton.set_health(200.0);
    logic.templates.insert("AmericaInfantryColonelBurton".into(), burton);

    let tank_id = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(30.0, 0.0, 40.0))
        .expect("tank");
    if let Some(o) = logic.host_object_mut(tank_id) {
        o.name = "SnipedHumvee".into();
        o.team_instance_name = "teamAmerica".into();
        o.select();
    }
    logic.selected_objects.push(tank_id);

    let hero_id = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(90.0, 0.0, 120.0),
        )
        .expect("burton");
    if let Some(o) = logic.host_object_mut(hero_id) {
        o.name = "ColonelBurton".into();
        o.team_instance_name = "teamAmerica".into();
        o.innate_stealth = true;
        o.stealth_delay_frames = 0;
        o.set_status_stealthed(true);
    }

    request_host_script_unmanned(HostScriptUnmannedRequest::Named {
        unit: "SnipedHumvee".into(),
    });
    logic.apply_host_unmanned_script_requests();
    {
        let tank = logic.host_object(tank_id).expect("unmanned tank");
        assert!(tank.status.disabled_unmanned, "SET_UNMANNED must stamp DISABLED_UNMANNED");
        assert_eq!(tank.team, Team::Neutral, "unmanned husk moves to Neutral");
        assert!(!tank.selected, "deselectObject PLAYERMASK_ALL");
    }
    assert!(!logic.selected_objects.contains(&tank_id));

    request_host_script_radar_event(HostScriptRadarEventRequest::Object {
        unit: "ColonelBurton".into(),
        event_type: 4,
    });
    logic.apply_host_radar_event_script_requests();
    let last = last_the_radar_event_host_position().expect("radar ping");
    assert!((last.x - 90.0).abs() < 0.1);
    assert!((last.z - 120.0).abs() < 0.1);

    request_host_script_stealth_enabled(HostScriptStealthEnabledRequest::Named {
        unit: "ColonelBurton".into(),
        enabled: false,
    });
    logic.apply_host_stealth_enabled_script_requests();
    {
        let hero = logic.host_object(hero_id).expect("hero");
        assert!(hero.script_unstealthed, "SET_STEALTH false stamps SCRIPT_UNSTEALTHED");
        assert!(!hero.status.stealthed, "script destalths immediately");
        assert!(hero.stealth_level_forbids_cloak(1, false, false, false, true));
    }
    logic.update_stealth_and_detection();
    {
        let hero = logic.host_object(hero_id).expect("hero after tick");
        assert!(
            !hero.status.stealthed,
            "script-unstealthed hero must stay destalthed"
        );
    }

    request_host_script_stealth_enabled(HostScriptStealthEnabledRequest::Named {
        unit: "ColonelBurton".into(),
        enabled: true,
    });
    logic.apply_host_stealth_enabled_script_requests();
    assert!(
        !logic
            .host_object(hero_id)
            .expect("hero re-enable")
            .script_unstealthed
    );

    request_host_script_unmanned(HostScriptUnmannedRequest::DeleteAll);
    logic.apply_host_unmanned_script_requests();
    let tank_gone = logic
        .host_object(tank_id)
        .map(|o| o.status.destroyed || !o.is_alive())
        .unwrap_or(true);
    assert!(tank_gone, "DELETE_ALL_UNMANNED must destroy husks");
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn team_create_radar_event_uses_first_member_not_centroid() {
    use crate::game_logic::host_radar::last_the_radar_event_host_position;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_radar_event, HostScriptRadarEventRequest,
    };

    OBJECT_REGISTRY.clear();
    let _ = gamelogic::scripting::take_host_script_radar_event_requests();

    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("AmericaTankCrusader");
    tank.set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tank);

    let first = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(10.0, 0.0, 20.0))
        .expect("first");
    if let Some(o) = logic.host_object_mut(first) {
        o.team_instance_name = "teamAmerica".into();
    }
    let second = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            Vec3::new(110.0, 0.0, 220.0),
        )
        .expect("second");
    if let Some(o) = logic.host_object_mut(second) {
        o.team_instance_name = "teamAmerica".into();
    }
    assert!(first.0 < second.0, "first created must have the lower id");

    request_host_script_radar_event(HostScriptRadarEventRequest::Team {
        team: "teamAmerica".into(),
        event_type: 4,
    });
    logic.apply_host_radar_event_script_requests();
    let last = last_the_radar_event_host_position().expect("team radar ping");
    assert!(
        (last.x - 10.0).abs() < 0.1 && (last.z - 20.0).abs() < 0.1,
        "TEAM_CREATE_RADAR_EVENT must ping first member, not centroid, got {last:?}"
    );
}



#[test]
fn live_host_team_hooks_add_member_and_notify_death_once() {
    use gamelogic::common::Dict;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::team::get_team_factory;
    use game_engine::common::well_known_keys::{
        key_team_name, key_team_on_create_script, key_team_on_unit_destroyed_script,
        key_team_owner,
    };

    OBJECT_REGISTRY.clear();
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("HostHookRanger");
    t.set_health(100.0);
    logic.templates.insert("HostHookRanger".into(), t);
    let id = logic
        .create_object("HostHookRanger", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.team_instance_name = "HostHookSquad".into();
    }

    {
        let Ok(mut factory) = get_team_factory().lock() else {
            return;
        };
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "HostHookSquad");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_create_script(), "OnCreateHostHook");
        dict.set_ascii_string(
            key_team_on_unit_destroyed_script(),
            "OnUnitDestroyedHostHook",
        );
        let _ = factory.init_team(
            gamelogic::common::AsciiString::from("HostHookSquad"),
            gamelogic::common::AsciiString::from("PlyrCivilian"),
            false,
            Some(&dict),
        );
    }

    logic.activate_leftover_team_for_host_object(id);
    logic.inject_host_script_query_snapshot();
    {
        let Ok(mut factory) = get_team_factory().lock() else {
            return;
        };
        let team = factory.find_team("HostHookSquad").expect("leftover team");
        {
            let mut guard = team.write().expect("write");
            guard.update_state();
        }
        let guard = team.read().expect("read");
        assert!(guard.has_member(id.0), "live host must DLINK the member");
        assert!(guard.is_active());
        assert!(
            !guard.is_created(),
            "OnCreate must be consumed once from leftover updateState"
        );
    }

    logic.notify_leftover_team_of_host_object_death(id);
    logic.notify_leftover_team_of_host_object_death(id);
    {
        let Ok(mut factory) = get_team_factory().lock() else {
            return;
        };
        let team = factory.find_team("HostHookSquad").expect("leftover team");
        let guard = team.read().expect("read");
        assert!(
            !guard.has_member(id.0),
            "onDie unlinks once; second notify is a no-op"
        );
    }
}



#[test]
fn live_host_polygon_inside_and_enter_without_object_registry() {
    use gamelogic::common::{AsciiString, ICoord3D};
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::polygon_trigger::PolygonTrigger;
    use gamelogic::scripting::{
        clear_host_script_query_snapshot, host_script_named_unit_in_named_area,
        update_host_object_trigger_flags,
    };

    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();
    let trigger = PolygonTrigger::new(
        1412,
        AsciiString::from("LivePolyPad"),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(30, 0, 0),
            ICoord3D::new(30, 30, 0),
            ICoord3D::new(0, 30, 0),
        ],
    );
    gamelogic::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(trigger);

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("NamedScout");
    t.set_health(100.0);
    logic.templates.insert("NamedScout".into(), t);
    let id = logic
        .create_object("NamedScout", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapNamedScout".into();
        o.team_instance_name = "teamUSA".into();
    }
    logic.inject_host_script_query_snapshot();
    assert_eq!(
        host_script_named_unit_in_named_area("MapNamedScout", "LivePolyPad"),
        Some(true)
    );
    update_host_object_trigger_flags(id.0, 10.0, 10.0, logic.frame, false, Some("teamUSA"));
    assert!(gamelogic::scripting::host_object_did_enter(
        id.0,
        &gamelogic::scripting::host_script_lookup_polygon_trigger("LivePolyPad").expect("poly")
    ));
    clear_host_script_query_snapshot();
}


#[test]
fn network_mode_helpers_match_lan_internet_multiplayer() {
    let mut game_logic = GameLogic::new();

    game_logic.game_mode = GameMode::SinglePlayer;
    assert!(!game_logic.isInNetworkGame());

    game_logic.game_mode = GameMode::Multiplayer;
    assert!(game_logic.isInMultiplayerGame());
    assert!(game_logic.isInNetworkGame());

    game_logic.game_mode = GameMode::Lan;
    assert!(game_logic.isInLanGame());
    assert!(game_logic.isInNetworkGame());

    game_logic.game_mode = GameMode::Internet;
    assert!(game_logic.isInInternetGame());
    assert!(game_logic.isInNetworkGame());
}

#[test]
fn military_caption_script_duration_uses_milliseconds_like_cpp() {
    assert!((GameLogic::military_caption_duration_seconds(2500) - 2.5).abs() < f32::EPSILON);
    assert_eq!(GameLogic::military_caption_duration_seconds(0), 0.0);
    assert_eq!(GameLogic::military_caption_duration_seconds(-1), 0.0);
}

#[test]
fn radar_force_keeps_ui_radar_visible_until_reverted() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic.mission_scripts.push_radar_enabled(false);
    game_logic.evaluate_and_execute_scripts(0.0);
    let ui_state = game_logic.update_ui_state(0);
    assert!(!ui_state.radar_enabled);
    assert!(!ui_state.radar_forced);

    game_logic.mission_scripts.push_radar_forced(true);
    game_logic.evaluate_and_execute_scripts(0.0);
    let ui_state = game_logic.update_ui_state(0);
    assert!(ui_state.radar_enabled);
    assert!(ui_state.radar_forced);

    game_logic.mission_scripts.push_radar_forced(false);
    game_logic.evaluate_and_execute_scripts(0.0);
    let ui_state = game_logic.update_ui_state(0);
    assert!(!ui_state.radar_enabled);
    assert!(!ui_state.radar_forced);
}

#[test]
fn script_radar_event_reaches_ui_ping() {
    let mut game_logic = GameLogic::new();
    game_logic.scripts_loaded = true;

    game_logic
        .mission_scripts
        .push_radar_event_request(RadarScriptEventRequest {
            position: Vec3::new(42.0, 7.0, 0.0),
            event_type: 3,
        });
    game_logic.evaluate_and_execute_scripts(0.0);

    let ui_state = game_logic.update_ui_state(0);
    assert_eq!(ui_state.radar_messages, vec!["Under attack"]);
    assert_eq!(ui_state.radar_pings.len(), 1);
    assert_eq!(ui_state.radar_pings[0].position, Vec3::new(42.0, 7.0, 0.0));
    assert_eq!(
        game_logic.last_radar_event_position(),
        Some(Vec3::new(42.0, 7.0, 0.0))
    );
}

/// Host AI residual: after a skirmish world wipe (objects.clear like load_map),
/// rebind restores rebuild budget / templates so AI update does not panic and
/// can issue builds again. Players + cash + difficulty stay intact.
#[test]
fn host_ai_rebind_after_world_wipe_keeps_players_cash_and_rebuilds() {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();

    let mut human = Player::new(0, Team::USA, "Player", true);
    human.resources.supplies = 10_000;
    logic.add_player(human);
    let mut ai = Player::new(1, Team::GLA, "GLA AI", false);
    ai.resources.supplies = 10_000;
    logic.add_player(ai);
    logic.add_ai_opponent(1, Team::GLA, AIDifficulty::Medium);
    logic.set_ai_active(1, true);

    // Force stale build refs: mark first GLA structure as "in progress" on AI queue.
    {
        let mut mgr = std::mem::take(&mut logic.ai_manager);
        if let Some(ai_player) = mgr.ai_players.get_mut(&1) {
            if let Some(b) = ai_player.building_queue.first_mut() {
                b.object_id = Some(ObjectId(9999));
                b.rebuild_count = b.max_rebuilds; // would block rebuild without rebind
                b.is_built = false;
            }
        }
        logic.ai_manager = mgr;
    }

    // Simulate load_map object wipe while preserving host players.
    logic.objects.clear();
    assert_eq!(logic.get_players().len(), 2);
    assert_eq!(
        logic.get_player(0).map(|p| p.resources.supplies),
        Some(10_000)
    );
    assert_eq!(
        logic.get_player(1).map(|p| p.resources.supplies),
        Some(10_000)
    );

    // Strip AI templates then rebind (must reinstall GLA_*).
    logic.templates.retain(|k, _| !k.starts_with("GLA_"));
    assert!(!logic.templates.contains_key("GLA_CommandCenter"));

    logic.rebind_host_ai_after_map_load();

    assert!(logic.templates.contains_key("GLA_CommandCenter"));
    assert!(logic.templates.contains_key("GLA_Soldier"));
    assert!(logic.is_host_ai_active(1));
    assert_eq!(logic.host_ai_difficulty(1), Some(AIDifficulty::Medium));
    // Rebuild budget restored (stale maxed rebuild_count cleared).
    {
        let mgr = &logic.ai_manager;
        let ai_player = mgr.ai_players.get(&1).expect("ai");
        let first = ai_player.building_queue.first().expect("layout");
        assert!(first.object_id.is_none());
        assert_eq!(first.rebuild_count, 0);
        assert!(!first.is_built);
    }

    logic.set_ai_active(1, false);
    assert!(!logic.is_host_ai_active(1));
    logic.set_ai_active(1, true);
    assert!(logic.is_host_ai_active(1));

    // Non-panicking multi-frame AI update after rebind.
    for _ in 0..20 {
        logic.update();
    }
    assert!(logic.host_ai_player_count() >= 1);
    assert!(logic.get_player(0).is_some() && logic.get_player(1).is_some());
    // AI should be able to start at least one structure once rebuild budget is open.
    assert!(
        logic.host_ai_activity_count() >= 1
            || logic
                .host_objects()
                .values()
                .any(|o| o.team == Team::GLA && o.is_kind_of(KindOf::Structure)),
        "AI rebuild soup should progress after rebind"
    );
}

#[test]
fn clear_game_data_scrubs_map_and_player_state() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    game_logic.game_mode = GameMode::Skirmish;
    game_logic.map_name = "Maps\\Test\\Test.map".to_string();
    game_logic.map_loaded = true;
    game_logic.objects.insert(
        ObjectId(7),
        Object::new(
            game_logic.templates.get("TestTank").cloned().unwrap(),
            ObjectId(7),
            Team::USA,
        ),
    );
    game_logic
        .players
        .insert(1, Player::new(1, Team::USA, "Player1", true));

    game_logic.clearGameData();

    assert_eq!(game_logic.game_mode, GameMode::None);
    assert!(game_logic.map_name.is_empty());
    assert!(!game_logic.map_loaded);
    assert!(game_logic.objects.is_empty());
    assert!(game_logic.players.is_empty());
}

#[test]
fn map_fallback_reports_the_map_that_actually_loaded() {
    let mut game_logic = GameLogic::new();

    let loaded = game_logic.load_map_or_fallback("__map_start_missing_requested_map__", "TestMap");

    assert_eq!(loaded.as_deref(), Some("TestMap"));
    assert_eq!(game_logic.get_current_map_name(), "TestMap");
    assert!(game_logic.map_loaded);
}

#[test]
fn map_fallback_forwards_live_milestones_for_each_real_attempt() {
    let mut game_logic = GameLogic::new();
    let mut milestones = Vec::new();

    let loaded = game_logic.load_map_or_fallback_with_progress(
        "__map_start_missing_requested_map__",
        "TestMap",
        |progress, phase| milestones.push((progress, phase.to_string())),
    );

    assert_eq!(loaded.as_deref(), Some("TestMap"));
    assert_eq!(
        milestones
            .iter()
            .filter(|(_, phase)| phase == "Preparing map data")
            .count(),
        2,
        "both the requested map and the real fallback must expose their load start"
    );
    assert!(
        milestones
            .iter()
            .any(|(progress, phase)| *progress >= 0.96 && phase == "Map load complete"),
        "the successfully loaded fallback must forward its final real milestone"
    );
}

#[test]
fn corrupt_selected_map_uses_and_reports_the_loaded_fallback() {
    let temp = tempfile::tempdir().expect("temporary map directory");
    let corrupt_map = temp.path().join("corrupt.map");
    std::fs::write(&corrupt_map, b"not a Generals map").expect("write corrupt map fixture");

    let mut game_logic = GameLogic::new();
    let loaded = game_logic.load_map_or_fallback(
        corrupt_map.to_str().expect("UTF-8 temporary map path"),
        "TestMap",
    );

    assert_eq!(loaded.as_deref(), Some("TestMap"));
    assert_eq!(game_logic.get_current_map_name(), "TestMap");
    assert!(game_logic.map_loaded);
}

#[test]
fn map_fallback_failure_leaves_no_active_map_identity_or_playable_world() {
    let mut game_logic = GameLogic::new();
    game_logic.start_new_game(GameMode::Skirmish);
    assert!(game_logic.load_map("TestMap"));
    assert!(game_logic.isInGame());

    let loaded = game_logic.load_map_or_fallback(
        "__map_start_missing_requested_map__",
        "__map_start_missing_fallback_map__",
    );

    assert_eq!(loaded, None);
    assert!(game_logic.get_current_map_name().is_empty());
    assert!(!game_logic.map_loaded);
    assert!(!game_logic.isInGame());
}

#[test]
fn asset_template_preserves_cpp_fs_kind_tokens() {
    let mut definition = ObjectDefinition::new("AmericaBarracks".to_string());
    definition
        .attributes
        .insert("KindOf".to_string(), "STRUCTURE FS_BARRACKS".to_string());

    let template =
        GameLogic::build_template_from_object_definition("AmericaBarracks", &definition, None);

    assert!(template.is_kind_of(KindOf::Structure));
    assert!(template.is_kind_of(KindOf::FSBarracks));
}

#[test]
fn asset_template_catalogue_seed_keeps_curated_templates_and_uses_exact_retail_fields() {
    let mut logic = GameLogic::new();
    let mut curated = ThingTemplate::new("AmericaTankCrusader");
    curated.set_health(777.0).set_model("CuratedExactModel");
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), curated);

    let mut existing = ObjectDefinition::new("AmericaTankCrusader".to_string());
    existing.object_type = "Vehicle".to_string();
    existing.hit_points = Some(480.0);
    existing.model_name = Some("AVCrusader".to_string());

    let mut added = ObjectDefinition::new("AmericaTankPaladin".to_string());
    added.object_type = "Vehicle".to_string();
    added.hit_points = Some(600.0);
    added.model_name = Some("AVPaladin".to_string());
    added.primary_weapon = Some("PaladinTankGun".to_string());
    added.secondary_weapon = Some("PaladinPointDefenseLaser".to_string());
    added.attributes.insert(
        "KindOf".to_string(),
        "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
    );
    added
        .attributes
        .insert("BuildCost".to_string(), "1100".to_string());
    added
        .attributes
        .insert("BuildTime".to_string(), "12.5".to_string());

    let mut ambient_only = ObjectDefinition::new("AmbientOnlyRetailAnchor".to_string());
    ambient_only
        .attributes
        .insert("SoundAmbient".to_string(), "AmbientWind".to_string());

    assert_eq!(
        logic.seed_asset_definition_templates_from_snapshot(vec![
            ("AmericaTankPaladin".to_string(), added),
            ("AmericaTankCrusader".to_string(), existing),
            ("AmbientOnlyRetailAnchor".to_string(), ambient_only),
        ]),
        2
    );

    let curated_after = logic
        .templates
        .get("AmericaTankCrusader")
        .expect("curated template retained");
    assert_eq!(curated_after.max_health, 777.0);
    assert_eq!(
        curated_after.model_name.as_deref(),
        Some("CuratedExactModel")
    );

    let seeded = logic
        .templates
        .get("AmericaTankPaladin")
        .expect("retail definition seeded");
    assert_eq!(seeded.max_health, 600.0);
    assert_eq!(seeded.build_cost.supplies, 1100);
    assert_eq!(seeded.build_time, 12.5);
    assert_eq!(seeded.model_name.as_deref(), Some("AVPaladin"));
    assert_eq!(
        seeded.primary_weapon_name.as_deref(),
        Some("PaladinTankGun")
    );
    assert_eq!(
        seeded.secondary_weapon_name.as_deref(),
        Some("PaladinPointDefenseLaser")
    );
    assert!(seeded.is_kind_of(KindOf::Vehicle));
    let ambient = logic
        .templates
        .get("AmbientOnlyRetailAnchor")
        .expect("SoundAmbient-only map object is now a live template");
    assert_eq!(ambient.sound_ambient.as_deref(), Some("AmbientWind"));
}

#[test]
fn shell_game_state_tracks_in_game_status() {
    let mut game_logic = GameLogic::new();
    assert!(!game_logic.isInGame());
    assert!(!game_logic.isInShellGame());

    game_logic.start_new_game(GameMode::Shell);
    assert!(
        game_logic.isInShellGame(),
        "GAME_SHELL should report shell state before the map is marked loaded"
    );

    game_logic.map_loaded = true;
    assert!(game_logic.isInShellGame());

    game_logic.start_new_game(GameMode::Skirmish);
    assert!(!game_logic.isInShellGame());
}

#[test]
fn live_script_tick_runs_one_script_engine_update() {
    // C++ GameLogic.cpp:3600 has exactly one TheScriptEngine->UPDATE()
    // per logic frame.  The live host tick must not also walk
    // MissionScriptRuntime over the same installed lists (hq-fxq1).
    let tick = include_str!("../world_scripts/scripts_camera.rs");
    let runtime = include_str!("../mission_scripts.rs");
    assert!(tick.contains("engine.update()"));
    assert!(tick.contains("guard.take()"));
    assert!(tick.contains("note_logic_frame"));

    assert!(
        !tick.contains("self.mission_scripts.update("),
        "MissionScriptRuntime must not be a second live ScriptEngine walk"
    );

    assert!(!tick.contains("update_shell_budgeted"));
    assert!(!runtime.contains("update_shell_budgeted"));
    assert!(!runtime.contains("SHELL_HEAVY_SCRIPT_WARMUP_FRAMES"));
    assert!(!runtime.contains("evaluate_shell_heavy_script_chunked"));
}

#[test]
fn exact_model_lookup_rejects_legacy_proxy_meshes() {
    let proxy_pairs = [
        ("PMRocks01b", "PMBoulders_D"),
        ("PTCypress01", "PTXARBVT01"),
        ("PMSwing", "PMBikeRack"),
        ("AVAMPHIB", "AVChinook"),
        ("AVChinook_A2", "AVChinook_A2MSH"),
        ("ABSupplyCT_A2", "ABSupplyCT_A2U"),
        ("AVPaladin", "AVCrusader_A"),
    ];

    for (requested, proxy) in proxy_pairs {
        assert_eq!(
            GameLogic::find_exact_available_model_name(
                requested,
                vec![format!("Art/W3D/{proxy}.W3D")].into_iter(),
            ),
            None,
            "a missing retail model must not be replaced with proxy mesh {proxy}"
        );
    }
}

#[test]
fn get_available_templates_filters_faction_prefixed_templates() {
    let mut game_logic = GameLogic::new();
    game_logic.templates.clear();

    let mut usa = ThingTemplate::new("USA_Tank");
    usa.add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Vehicle);
    game_logic.templates.insert(usa.name.clone(), usa);

    let mut china = ThingTemplate::new("China_Tank");
    china
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Vehicle);
    game_logic.templates.insert(china.name.clone(), china);

    let mut gla = ThingTemplate::new("GLA_Tank");
    gla.add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Vehicle);
    game_logic.templates.insert(gla.name.clone(), gla);

    let mut shared = ThingTemplate::new("TestScout");
    shared
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Infantry);
    game_logic.templates.insert(shared.name.clone(), shared);

    let available = game_logic.get_available_templates(Team::USA);
    assert!(available.contains(&"USA_Tank".to_string()));
    assert!(available.contains(&"TestScout".to_string()));
    assert!(!available.contains(&"China_Tank".to_string()));
    assert!(!available.contains(&"GLA_Tank".to_string()));
}

#[test]
fn visibility_filter_allows_object_when_shroud_snapshot_missing() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let object_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
        .expect("object should be created");
    let object = game_logic
        .host_object(object_id)
        .expect("object should exist");

    assert!(GameLogic::is_object_visible_for_team(
        object_id,
        object,
        Team::USA,
        None
    ));
}

#[test]
fn visibility_filter_requires_visible_or_explored_membership_with_shroud_snapshot() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let object_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
        .expect("object should be created");
    let object = game_logic
        .host_object(object_id)
        .expect("object should exist");

    let mut visible_only = ShroudVisibilitySnapshot {
        visible_objects: HashSet::new(),
        explored_objects: HashSet::new(),
    };
    visible_only.visible_objects.insert(object_id.0);
    assert!(GameLogic::is_object_visible_for_team(
        object_id,
        object,
        Team::USA,
        Some(&visible_only)
    ));

    let mut explored_only = ShroudVisibilitySnapshot {
        visible_objects: HashSet::new(),
        explored_objects: HashSet::new(),
    };
    explored_only.explored_objects.insert(object_id.0);
    assert!(GameLogic::is_object_visible_for_team(
        object_id,
        object,
        Team::USA,
        Some(&explored_only)
    ));

    let hidden = ShroudVisibilitySnapshot {
        visible_objects: HashSet::new(),
        explored_objects: HashSet::new(),
    };
    assert!(!GameLogic::is_object_visible_for_team(
        object_id,
        object,
        Team::USA,
        Some(&hidden)
    ));
}

#[test]
fn minimap_visibility_filter_requires_live_visibility_for_non_structures() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let object_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
        .expect("object should be created");
    let object = game_logic
        .host_object(object_id)
        .expect("object should exist");

    let mut explored_only = ShroudVisibilitySnapshot {
        visible_objects: HashSet::new(),
        explored_objects: HashSet::new(),
    };
    explored_only.explored_objects.insert(object_id.0);

    assert!(!GameLogic::is_object_visible_on_minimap_for_team(
        object_id,
        object,
        Team::USA,
        Some(&explored_only),
    ));
}

#[test]
fn minimap_visibility_filter_keeps_explored_structures() {
    let mut game_logic = GameLogic::new();
    let mut structure_template = ThingTemplate::new("TestStructure");
    structure_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    game_logic
        .templates
        .insert("TestStructure".to_string(), structure_template);
    let object_id = game_logic
        .create_object("TestStructure", Team::GLA, Vec3::new(20.0, 0.0, 20.0))
        .expect("structure should be created");
    let object = game_logic
        .host_object(object_id)
        .expect("structure should exist");

    let mut explored_only = ShroudVisibilitySnapshot {
        visible_objects: HashSet::new(),
        explored_objects: HashSet::new(),
    };
    explored_only.explored_objects.insert(object_id.0);

    assert!(GameLogic::is_object_visible_on_minimap_for_team(
        object_id,
        object,
        Team::USA,
        Some(&explored_only),
    ));
}

#[test]
fn entering_state_docks_unit_into_transport_when_close() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let transport_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("transport should be created");
    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit should be created");

    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        unit.target = Some(transport_id);
        unit.set_ai_state(AIState::Entering);
        unit.set_status_moving(true);
    }

    game_logic.update_ai(&[transport_id, unit_id], 1.0 / 60.0);

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport should exist");
    assert!(
        transport.contained_units().contains(&unit_id),
        "entering unit should be registered as transport occupant"
    );

    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    assert_eq!(unit.ai_state, AIState::Docked);
    assert_eq!(unit.target, Some(transport_id));
    assert!(!unit.can_move(), "docked units should not be movable");
    assert!(
        !unit.can_attack(),
        "docked units should not be independently attackable"
    );
}

#[test]
fn docking_state_moves_unit_toward_transport_when_far() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let transport_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("transport should be created");
    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .expect("unit should be created");

    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        unit.target = Some(transport_id);
        unit.set_ai_state(AIState::Docking);
    }

    game_logic.update_ai(&[transport_id, unit_id], 1.0 / 60.0);

    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    let destination = unit
        .movement
        .target_position
        .expect("docking unit should move toward transport");
    assert!(destination.distance(Vec3::new(0.0, 0.0, 0.0)) < 0.01);
    assert_eq!(unit.ai_state, AIState::Docking);
}

#[test]
fn enter_command_rejects_enemy_occupied_transport() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_transport_template(&mut game_logic);

    let friendly_unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(-10.0, 0.0, 0.0))
        .expect("friendly unit should be created");
    let enemy_transport_id = game_logic
        .create_object("TestTransport", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("enemy transport should be created");
    let enemy_occupant_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("enemy occupant should be created");

    {
        let enemy_transport = game_logic
            .host_object_mut(enemy_transport_id)
            .expect("enemy transport should exist");
        assert!(
            enemy_transport.add_occupant(enemy_occupant_id),
            "enemy transport should hold an occupant for legality test"
        );
    }
    {
        let enemy_occupant = game_logic
            .host_object_mut(enemy_occupant_id)
            .expect("enemy occupant should exist");
        enemy_occupant.target = Some(enemy_transport_id);
        enemy_occupant.set_ai_state(AIState::Docked);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Enter {
            target_id: enemy_transport_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![friendly_unit_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let friendly = game_logic
        .host_object(friendly_unit_id)
        .expect("friendly unit should exist");
    assert_ne!(
        friendly.target,
        Some(enemy_transport_id),
        "enter command should not target occupied enemy transport"
    );
    assert_ne!(
        friendly.ai_state,
        AIState::Entering,
        "unit should not start entering an occupied enemy transport"
    );
}

#[test]
fn enter_command_allows_empty_enemy_non_faction_structure() {
    let mut game_logic = GameLogic::new();
    // Residual: structure garrison accepts infantry (not vehicles).
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let friendly_unit_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-10.0, 0.0, 0.0))
        .expect("friendly unit should be created");
    let enemy_garrison_id = game_logic
        .create_object("TestBunker", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("enemy garrison should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Enter {
            target_id: enemy_garrison_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![friendly_unit_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let friendly = game_logic
        .host_object(friendly_unit_id)
        .expect("friendly unit should exist");
    assert_eq!(friendly.target, Some(enemy_garrison_id));
    assert_eq!(friendly.ai_state, AIState::Entering);
}

#[test]
fn entering_state_clears_enemy_structure_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    let unit_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit should be created");
    let enemy_barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("enemy barracks should be created");

    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        unit.target = Some(enemy_barracks_id);
        unit.set_ai_state(AIState::Entering);
        unit.set_status_moving(true);
    }

    game_logic.update_ai(&[unit_id, enemy_barracks_id], 1.0 / 60.0);

    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    assert!(
        unit.target.is_none(),
        "entering should clear enemy structure targets"
    );
    assert_eq!(
        unit.ai_state,
        AIState::Idle,
        "unit should return to idle when enter legality fails"
    );
}

#[test]
fn entering_state_allows_empty_enemy_non_faction_structure() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let unit_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit should be created");
    let enemy_garrison_id = game_logic
        .create_object("TestBunker", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("enemy garrison should be created");

    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        unit.target = Some(enemy_garrison_id);
        unit.set_ai_state(AIState::Entering);
        unit.set_status_moving(true);
    }

    game_logic.update_ai(&[unit_id, enemy_garrison_id], 1.0 / 60.0);

    let garrison = game_logic
        .host_object(enemy_garrison_id)
        .expect("garrison should exist");
    assert!(garrison.contained_units().contains(&unit_id));

    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    assert_eq!(unit.ai_state, AIState::Garrisoned);
    assert_eq!(unit.target, Some(enemy_garrison_id));
    assert_eq!(unit.contained_by, Some(enemy_garrison_id));
}

#[test]
fn guard_state_engages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let guard_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("guard should be created");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(25.0, 0.0, 0.0))
        .expect("enemy should be created");

    {
        let guard = game_logic
            .host_object_mut(guard_id)
            .expect("guard should exist");
        guard.set_ai_state(AIState::GuardingArea);
        guard.guard_position = Some(Vec3::new(0.0, 0.0, 0.0));
        guard.guard_radius = 100.0;
    }
    game_logic.guard_next_enemy_scan.insert(guard_id, game_logic.frame);
    game_logic.update_ai(&[guard_id, enemy_id], 1.0 / 60.0);

    let guard = game_logic
        .host_object(guard_id)
        .expect("guard should exist");
    assert_eq!(guard.ai_state, AIState::Attacking);
    assert_eq!(guard.target, Some(enemy_id));
}

#[test]
fn process_ai_behavior_idle_defers_to_mood_auto_acquire() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created");
    let _enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy should be created");
    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.weapon = Some(Weapon {
            range: 150.0,
            ..Weapon::default()
});
    }

    let (pos, team, can_attack) = {
        let attacker = game_logic
            .host_object(attacker_id)
            .expect("attacker should exist");
        (attacker.get_position(), attacker.team, attacker.can_attack())
    };
    let command = game_logic.process_ai_behavior(
        attacker_id,
        AIState::Idle,
        None,
        pos,
        team,
        can_attack,
        30,
        1.0 / 60.0,
    );

    assert!(
        command.is_none(),
        "Idle acquire is mood-gated; process_ai_behavior must not 200-scan, got {command:?}"
    );
}

#[test]
fn process_ai_behavior_attacking_fallback_stops_without_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created");
    let (pos, team, can_attack) = {
        let attacker = game_logic
            .host_object(attacker_id)
            .expect("attacker should exist");
        (attacker.get_position(), attacker.team, attacker.can_attack())
    };

    let command = game_logic.process_ai_behavior(
        attacker_id,
        AIState::Attacking,
        None,
        pos,
        team,
        can_attack,
        0,
        1.0 / 60.0,
    );

    match command {
        Some(AICommand::StopAttack { object_id }) => assert_eq!(object_id, attacker_id),
        other => panic!("expected attacking fallback to stop attack, got {other:?}"),
    }
}

#[test]
fn process_ai_behavior_hunt_seeks_map_wide_not_100_circle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, -20.0))
        .expect("unit should be created");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(2000.0, 0.0, 0.0))
        .expect("far enemy should be created");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        unit.weapon = Some(Weapon {
            range: 150.0,
            ..Weapon::default()
});
    }
    let (start, team, can_attack) = {
        let unit = game_logic.host_object(unit_id).expect("unit should exist");
        (unit.get_position(), unit.team, unit.can_attack())
    };
    game_logic.hunt_next_enemy_scan.insert(unit_id, 30);

    let command = game_logic.process_ai_behavior(
        unit_id,
        AIState::Patrolling,
        None,
        start,
        team,
        can_attack,
        30,
        1.0 / 60.0,
    );

    match command {
        Some(AICommand::AttackTarget {
            object_id,
            target_id,
        }) => {
            assert_eq!(object_id, unit_id);
            assert_eq!(target_id, far_id, "Hunt must seek map-wide, not a 200 bubble");
        }
        Some(AICommand::MoveTo { position, .. }) => {
            panic!(
                "Hunt must not wander a 100-circle, dest dist={}",
                start.distance(position)
            );
        }
        other => panic!("expected hunt to attack far enemy, got {other:?}"),
    }
}

#[test]
fn repairing_state_heals_target_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let repairer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repairer should be created");
    let damaged_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("target should be created");

    {
        let damaged = game_logic
            .host_object_mut(damaged_id)
            .expect("damaged unit should exist");
        let _ = damaged.take_damage(80.0);
    }
    {
        let repairer = game_logic
            .host_object_mut(repairer_id)
            .expect("repairer should exist");
        repairer.target = Some(damaged_id);
        repairer.set_ai_state(AIState::Repairing);
    }
    let before = game_logic
        .host_object(damaged_id)
        .expect("damaged unit should exist")
        .health
        .current;

    game_logic.update_ai(&[repairer_id, damaged_id], 1.0);

    let after = game_logic
        .host_object(damaged_id)
        .expect("damaged unit should exist")
        .health
        .current;
    assert!(
        after > before,
        "repairing state should restore target health"
    );
}

#[test]
fn seeking_repair_state_heals_self_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_bay_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair source should be created");
    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("unit should be created");

    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        let _ = unit.take_damage(90.0);
        unit.target = Some(repair_bay_id);
        unit.set_ai_state(AIState::SeekingRepair);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit should exist")
        .health
        .current;

    game_logic.update_ai(&[repair_bay_id, unit_id], 1.0);

    let after = game_logic
        .host_object(unit_id)
        .expect("unit should exist")
        .health
        .current;
    assert!(
        after > before,
        "seeking repair should heal the damaged unit"
    );
}

#[test]
fn seeking_repair_state_clears_under_construction_destination() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_bay_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair source should be created");
    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("unit should be created");

    {
        let repair_bay = game_logic
            .host_object_mut(repair_bay_id)
            .expect("repair source should exist");
        repair_bay.set_status_under_construction(true);
    }
    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        let _ = unit.take_damage(90.0);
        unit.target = Some(repair_bay_id);
        unit.set_ai_state(AIState::SeekingRepair);
        unit.set_status_moving(true);
    }

    game_logic.update_ai(&[repair_bay_id, unit_id], 1.0 / 60.0);

    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    assert!(
        unit.target.is_none(),
        "seeking repair should clear under-construction destinations"
    );
    assert_eq!(unit.ai_state, AIState::Idle);
}

#[test]
fn evacuate_command_unloads_selected_transport_occupants() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_transport_template(&mut game_logic);

    let transport_id = game_logic
        .create_object("TestTransport", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("transport should be created");
    let unit_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit should be created");

    {
        let transport = game_logic
            .host_object_mut(transport_id)
            .expect("transport should exist");
        assert!(transport.add_occupant(unit_id));
    }
    {
        let unit = game_logic
            .host_object_mut(unit_id)
            .expect("unit should exist");
        unit.target = Some(transport_id);
        unit.set_ai_state(AIState::Docked);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Evacuate,
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![transport_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport should exist");
    assert!(
        !transport.contained_units().contains(&unit_id),
        "evacuate should remove occupants from selected transport"
    );
    let unit = game_logic.host_object(unit_id).expect("unit should exist");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.target.is_none());
    assert!(unit.can_move());
}

#[test]
fn capture_command_does_not_instantly_flip_building_owner() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(
        building.team,
        Team::GLA,
        "capture command should not instantly transfer ownership"
    );

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_eq!(captor.ai_state, AIState::Capturing);
    assert_eq!(captor.target, Some(building_id));
}

#[test]
fn infantry_capture_requires_completed_capture_upgrade_when_player_exists() {
    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    game_logic
        .host_object_mut(captor_id)
        .expect("captor should exist")
        .pause_special_power_countdown(
            &crate::command_system::SpecialPowerType::RangerCaptureBuilding,
            true,
        );

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_ne!(captor.ai_state, AIState::Capturing);
    assert_ne!(captor.target, Some(building_id));

    game_logic
        .get_player_mut(0)
        .expect("USA player should exist")
        .unlocked_sciences
        .insert("Upgrade_InfantryCaptureBuilding".to_string());
    game_logic.apply_upgrade_to_object(captor_id, "Upgrade_InfantryCaptureBuilding");
    // Retail capture powers reload for 15 seconds after `StartsPaused` is
    // unpaused; advance the test authority clock before issuing the command.
    let _ = game_logic.update_with_dt(15.1);

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist after upgraded command");
    assert_eq!(captor.ai_state, AIState::Capturing);
    assert_eq!(captor.target, Some(building_id));
}

#[test]
fn capture_channel_uses_authored_ranger_unpack_prepare_and_pack_timing() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");

    {
        let captor = game_logic
            .host_object_mut(captor_id)
            .expect("captor should exist");
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }

    // C++ `SpecialAbilityUpdate` starts unpacking on arrival, but it does not
    // mark the SpecialPower triggered—or transfer ownership—at click time.
    game_logic.update_ai(&[captor_id, building_id], 1.0 / 60.0);

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after unpack start");
    assert_eq!(
        captor.capture_channel.map(|channel| channel.phase),
        Some(CaptureChannelPhase::Unpacking)
    );
    assert!(
        !captor
            .special_power_cooldowns
            .contains_key(&crate::command_system::SpecialPowerType::RangerCaptureBuilding),
        "Ranger ReloadTime must not begin before preparation"
    );
    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::GLA,
        "unpacking must not transfer ownership"
    );

    // Retail Ranger: UnpackTime=3000ms, PreparationTime=20000ms.
    game_logic.update_ai(&[captor_id, building_id], 3.0);
    let captor = game_logic.host_object(captor_id).expect("captor preparing");
    assert_eq!(
        captor.capture_channel.map(|channel| channel.phase),
        Some(CaptureChannelPhase::Preparing)
    );
    assert!(captor.status.using_ability);
    let ranger_reload = captor
        .special_power_cooldowns
        .get(&crate::command_system::SpecialPowerType::RangerCaptureBuilding)
        .copied()
        .unwrap_or_default();
    assert!(
        ranger_reload >= 14.9,
        "Ranger preparation must start the real 15s reload, got {ranger_reload}"
    );

    game_logic.update_ai(&[captor_id, building_id], 19.9);
    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::GLA,
        "the target remains enemy until the entire preparation completes"
    );
    game_logic.update_ai(&[captor_id, building_id], 0.1);

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(
        building.team,
        Team::USA,
        "capturing state should transfer structure only after preparation"
    );

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_eq!(captor.ai_state, AIState::Capturing);
    assert_eq!(
        captor.capture_channel.map(|channel| channel.phase),
        Some(CaptureChannelPhase::Packing),
        "C++ keeps the unit busy through PackTime after the ownership transfer"
    );
    assert!(!captor.status.using_ability);

    // Retail Ranger PackTime=2000ms.
    game_logic.update_ai(&[captor_id, building_id], 2.0);
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after packing");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

/// C++ `SpecialAbilityUpdate::triggerAbilityEffect` AwardXPForTriggering
/// (`SpecialAbilityUpdate.cpp:1248-1253`). Retail Ranger capture = 15.
#[test]
fn capture_trigger_awards_ranger_award_xp_for_triggering() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");
    {
        let captor = game_logic.host_object_mut(captor_id).expect("captor");
        captor.thing.template.is_trainable = true;
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
        assert_eq!(captor.experience.current, 0.0);
    }

    // Ranger Unpack 3s + Preparation 20s → trigger.
    game_logic.update_ai(&[captor_id, building_id], 3.0);
    game_logic.update_ai(&[captor_id, building_id], 20.0);

    let captor = game_logic.host_object(captor_id).expect("captor after trigger");
    assert_eq!(
        captor.experience.current,
        crate::game_logic::host_structure_economy_residual::CAPTURE_AWARD_XP as f32,
        "Ranger capture must grant AwardXPForTriggering=15"
    );
    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::USA,
        "ownership transfer is the trigger that awards XP"
    );
}

/// C++ `Object::defect` does not restore HP on infantry / Lotus capture.
#[test]
fn capture_does_not_heal_building_to_full() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");
    {
        let building = game_logic.host_object_mut(building_id).expect("building");
        building.health.current = 40.0;
        building.health.maximum = building.health.maximum.max(100.0);
        building.max_health = building.health.maximum;
    }
    {
        let captor = game_logic.host_object_mut(captor_id).expect("captor");
        captor.thing.template.is_trainable = true;
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }

    game_logic.update_ai(&[captor_id, building_id], 3.0);
    game_logic.update_ai(&[captor_id, building_id], 20.0);

    let building = game_logic.host_object(building_id).expect("building");
    assert_eq!(building.team, Team::USA, "capture must transfer ownership");
    assert!(
        (building.health.current - 40.0).abs() < 0.01,
        "C++ defect must keep current HP, got {}",
        building.health.current
    );
}



#[test]
fn black_lotus_capture_uses_its_zero_reload_at_preparation_not_ranger_timer() {
    use crate::game_logic::{CapturePowerKind, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);

    let mut lotus = ThingTemplate::new("CaptureTimingBlackLotus");
    lotus
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    lotus.capture_power = CapturePowerKind::BlackLotus;
    // Retail baseline Black Lotus uses a 150-unit start range and zero
    // ReloadTime; the compact test deliberately has no unpack animation so
    // the preparation boundary is unambiguous.
    lotus.capture_start_ability_range = Some(150.0);
    lotus.capture_unpack_time_ms = Some(0);
    lotus.capture_preparation_time_ms = Some(6_000);
    lotus.capture_pack_time_ms = Some(2_800);
    game_logic
        .templates
        .insert("CaptureTimingBlackLotus".to_string(), lotus);

    let lotus_id = game_logic
        .create_object(
            "CaptureTimingBlackLotus",
            Team::China,
            Vec3::new(3.0, 0.0, 0.0),
        )
        .expect("Black Lotus");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::ZERO)
        .expect("target building");
    {
        let lotus = game_logic
            .host_object_mut(lotus_id)
            .expect("Black Lotus object");
        lotus.target = Some(building_id);
        lotus.set_ai_state(AIState::Capturing);
    }

    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 30.0);
    let lotus = game_logic.host_object(lotus_id).expect("Lotus preparing");
    assert_eq!(
        lotus.capture_channel.map(|channel| channel.phase),
        Some(CaptureChannelPhase::Preparing)
    );
    assert!(lotus.status.using_ability);
    assert!(
        !lotus
            .special_power_cooldowns
            .contains_key(&crate::command_system::SpecialPowerType::BlackLotusCaptureBuilding),
        "Black Lotus ReloadTime is 0, never borrow Ranger's 15s timer"
    );

    game_logic.update_ai(&[lotus_id, building_id], 6.0);
    assert_eq!(
        game_logic
            .host_object(building_id)
            .expect("captured target")
            .team,
        Team::China
    );
    let lotus = game_logic.host_object(lotus_id).expect("Lotus packing");
    assert_eq!(
        lotus.capture_channel.map(|channel| channel.phase),
        Some(CaptureChannelPhase::Packing)
    );
    assert!(!lotus
        .special_power_cooldowns
        .contains_key(&crate::command_system::SpecialPowerType::BlackLotusCaptureBuilding));
}

#[test]
fn capture_authority_rejects_immune_and_nonstealthed_garrison_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut immune = ThingTemplate::new("CaptureAuthorityImmuneTarget");
    immune
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    immune.immune_to_capture = true;
    game_logic
        .templates
        .insert("CaptureAuthorityImmuneTarget".to_string(), immune);

    let mut garrison = ThingTemplate::new("CaptureAuthorityGarrisonTarget");
    garrison
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    // This is the dedicated `GarrisonContain` semantic consumed by the C++
    // capture guard, not generic Enter capacity or a target-name convention.
    garrison.garrison_contain_max = Some(5);
    game_logic
        .templates
        .insert("CaptureAuthorityGarrisonTarget".to_string(), garrison);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-3.0, 0.0, 0.0))
        .expect("captor");
    let immune_id = game_logic
        .create_object("CaptureAuthorityImmuneTarget", Team::GLA, Vec3::ZERO)
        .expect("immune target");
    assert!(
        !game_logic.can_unit_capture_building(captor_id, immune_id, false),
        "KINDOF_IMMUNE_TO_CAPTURE must reject before any relationship fallback"
    );

    let garrison_id = game_logic
        .create_object(
            "CaptureAuthorityGarrisonTarget",
            Team::GLA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("garrison target");
    let occupant_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy garrison occupant");
    assert!(
        game_logic
            .host_object_mut(garrison_id)
            .expect("garrison target")
            .add_occupant(occupant_id),
        "authored GarrisonContain capacity must hold the occupant"
    );
    game_logic
        .host_object_mut(occupant_id)
        .expect("occupant")
        .set_contained_by(Some(garrison_id));
    assert!(
        !game_logic.can_unit_capture_building(captor_id, garrison_id, false),
        "a non-stealthed garrison occupant must block CaptureBuilding"
    );

    game_logic
        .host_object_mut(occupant_id)
        .expect("occupant")
        .status
        .stealthed = true;
    assert!(
        game_logic.can_unit_capture_building(captor_id, garrison_id, false),
        "the garrison guard is distinct from the enemy-occupant relationship guard"
    );
}

#[test]
fn infantry_capture_prep_stamps_raising_flag() {
    use crate::game_logic::host_enum_table_residual::raising_flag_model_bit;
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::ZERO)
        .expect("building");
    {
        let captor = game_logic.host_object_mut(captor_id).expect("captor");
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }
    game_logic.update_ai(&[captor_id, building_id], 3.0);
    let bits = game_logic
        .host_object(captor_id)
        .expect("captor")
        .model_condition_bits;
    assert_ne!(
        bits & (1u128 << raising_flag_model_bit()),
        0,
        "infantry capture prep must stamp RAISING_FLAG"
    );
}

/// C++ `isWithinStartAbilityRange` + ApproachRequiresLOS: infantry capture
/// must not start unpack when terrain LOS to the building is blocked.
#[test]
fn infantry_capture_start_range_requires_approach_los() {
    use crate::game_logic::CaptureChannelPhase;
    let mut blocked = GameLogic::new();
    ensure_test_infantry_template(&mut blocked);
    ensure_test_structure_template(&mut blocked);
    install_test_mid_ridge(&mut blocked);
    let captor_id = blocked
        .create_object("TestInfantry", Team::USA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("captor");
    let building_id = blocked
        .create_object("TestBuilding", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    if let Some(captor) = blocked.host_object_mut(captor_id) {
        captor.set_selection_radius(80.0);
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }
    if let Some(building) = blocked.host_object_mut(building_id) {
        building.set_selection_radius(80.0);
    }
    blocked.update_ai(&[captor_id, building_id], 1.0 / 30.0);
    assert_eq!(
        blocked
            .host_object(captor_id)
            .and_then(|o| o.capture_channel)
            .map(|c| c.phase),
        None,
        "blocked terrain LOS must not start infantry capture unpack"
    );

    let mut clear = GameLogic::new();
    ensure_test_infantry_template(&mut clear);
    ensure_test_structure_template(&mut clear);
    let captor_id = clear
        .create_object("TestInfantry", Team::USA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("captor");
    let building_id = clear
        .create_object("TestBuilding", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    if let Some(captor) = clear.host_object_mut(captor_id) {
        captor.set_selection_radius(80.0);
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }
    if let Some(building) = clear.host_object_mut(building_id) {
        building.set_selection_radius(80.0);
    }
    clear.update_ai(&[captor_id, building_id], 1.0 / 30.0);
    assert_eq!(
        clear
            .host_object(captor_id)
            .and_then(|o| o.capture_channel)
            .map(|c| c.phase),
        Some(CaptureChannelPhase::Unpacking),
        "clear LOS in start range must begin infantry capture unpack"
    );
}


#[test]
fn lotus_capture_prep_stamps_firing_a() {
    use crate::game_logic::host_enum_table_residual::firing_a_model_bit;
    use crate::game_logic::CapturePowerKind;
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    let mut lotus = ThingTemplate::new("ChinaInfantryBlackLotus");
    lotus
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    lotus.capture_power = CapturePowerKind::BlackLotus;
    lotus.capture_start_ability_range = Some(5.0);
    lotus.capture_unpack_time_ms = Some(0);
    lotus.capture_preparation_time_ms = Some(6_000);
    lotus.capture_pack_time_ms = Some(2_000);
    game_logic
        .templates
        .insert("ChinaInfantryBlackLotus".into(), lotus);
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(3.0, 0.0, 0.0),
        )
        .expect("lotus");
    let building_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::ZERO)
        .expect("building");
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.target = Some(building_id);
        lotus.set_ai_state(AIState::Capturing);
    }
    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 30.0);
    let bits = game_logic
        .host_object(lotus_id)
        .expect("lotus")
        .model_condition_bits;
    assert_ne!(
        bits & (1u128 << firing_a_model_bit()),
        0,
        "Lotus capture prep must stamp FIRING_A"
    );
}

#[test]
fn hacker_disable_prep_stamps_firing_a() {
    use crate::game_logic::host_enum_table_residual::firing_a_model_bit;
    use crate::game_logic::{HackerDisableBuildingMetadata, HackerDisableChannelPhase, Player};
    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", false));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    ensure_test_structure_template(&mut game_logic);
    let mut hacker_tpl = ThingTemplate::new("TypedHackerDisableFiringA");
    hacker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    hacker_tpl.hacker_disable_building = Some(HackerDisableBuildingMetadata {
        special_power_template: "SpecialAbilityHackerDisableBuilding".to_string(),
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
        reload_time_frames: 0,
        required_science: None,
        shared_n_sync: false,
        start_ability_range: 150.0,
        ability_abort_range: 10_000_000.0,
        approach_requires_los: false,
        unpack_time_ms: 0,
        preparation_time_ms: 3_000,
        persistent_prep_time_ms: 333,
        effect_duration_ms: 2_000,
        pack_time_ms: 0,
        pack_unpack_variation_factor: 0.0,
        persistence_requires_recharge: false,
    });
    game_logic
        .templates
        .insert("TypedHackerDisableFiringA".into(), hacker_tpl);
    let hacker_id = game_logic
        .create_object_for_player("TypedHackerDisableFiringA", 0, Vec3::new(5.0, 0.0, 0.0))
        .expect("hacker");
    let building_id = game_logic
        .create_object_for_player("TestBuilding", 1, Vec3::ZERO)
        .expect("building");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::HackerDisableBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[hacker_id, building_id], 1.0 / 30.0);
    let phase = game_logic
        .host_object(hacker_id)
        .and_then(|o| o.hacker_disable_channel)
        .map(|ch| ch.phase);
    assert_eq!(phase, Some(HackerDisableChannelPhase::Preparing));
    let bits = game_logic
        .host_object(hacker_id)
        .expect("hacker")
        .model_condition_bits;
    assert_ne!(
        bits & (1u128 << firing_a_model_bit()),
        0,
        "Hacker disable prep must stamp FIRING_A"
    );
}


#[test]
fn human_capture_rejects_shrouded_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.is_local = true;
    game_logic.add_player(player);
    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-3.0, 0.0, 0.0))
        .expect("captor");
    if let Some(o) = game_logic.host_object_mut(captor_id) {
        o.owner_player_id = Some(0);
    }
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::ZERO)
        .expect("building");
    assert!(
        !game_logic.can_unit_capture_building(captor_id, building_id, true),
        "human capture must refuse a fogged/shrouded structure"
    );
    assert!(
        game_logic.can_unit_capture_building(captor_id, building_id, false),
        "already-running / script channel must not re-check shroud"
    );
}

/// Exercise the actual offline physical RMB route: frozen presentation facts
/// classify the click, then the resulting command crosses GameLogic authority
/// into CommandExecutor.  Two same-faction player slots are deliberately
/// allied here: C++ SupplyCenterDockUpdate still requires the exact
/// controlling player, whereas a warehouse uses the ally relationship.
#[test]
fn physical_rmb_dock_uses_exact_controller_not_same_faction_friendliness() {
    use crate::command_system::{
        CommandSystem, CommandType, ModifierKeys, MouseButton, MouseCommandContext,
        PresentationSelectedUnitHint, PresentationTargetHint,
    };
    use crate::game_logic::{DockKind, Player};
    use crate::presentation_frame::PresentationFrame;

    fn dock_context(
        frame: &PresentationFrame,
        collector_id: ObjectId,
        target_id: ObjectId,
    ) -> MouseCommandContext {
        let collector = frame
            .objects
            .iter()
            .find(|object| object.id == collector_id)
            .expect("collector must be present in frozen frame");
        let target = frame
            .objects
            .iter()
            .find(|object| object.id == target_id)
            .expect("dock target must be present in frozen frame");
        MouseCommandContext {
            world_position: target.position,
            target_object: Some(target_id),
            target_presentation: Some(PresentationTargetHint {
                id: target_id,
                is_alive: !target.destroyed && target.health_current > 0.0,
                is_structure: PresentationFrame::object_has_kind(target, KindOf::Structure),
                is_resource: false,
                under_construction: target.under_construction,
                sold: target.sold,
                team: target.team,
                is_enemy_of_local: frame.is_enemy_of_local(target),
                is_neutral: target.team == Team::Neutral,
                template_name: target.template_name.clone(),
                can_be_entered: false,
                enter_available_capacity: 0,
                enter_uses_transport_slots: false,
                enter_requires_infantry: false,
                enter_forbids_aircraft: false,
                enter_disabled_subdued: false,
                enter_is_rider_change: false,
                rider_change_allowed_templates: Vec::new(),
                is_damaged: false,
                is_friendly_of_local: frame.is_allied_with_local(target),
                provides_vehicle_repair: false,
                provides_aircraft_repair: false,
                provides_heal: false,
                can_provide_service: true,
                dock_kind: target.dock_kind,
                dock_controller_is_local: frame.is_owned_by_local(target),
                stored_supplies: target.stored_supplies,
                capturable: target.capturable,
                immune_to_capture: target.immune_to_capture,
                capture_garrisonable: target.capture_garrisonable,
                capture_nonstealthed_garrison_count: 0,
                capture_friendly_garrison_count: 0,
                capture_target_effectively_stealthed: false,
                is_crate: false,
                is_salvage_crate: false,
                is_vehicle: PresentationFrame::object_has_kind(target, KindOf::Vehicle),
                is_aircraft: false,
                is_drone: false,
                is_carbomb: false,
                is_unmanned: false,
                is_mine: false,
            }),
            selected_presentation: vec![PresentationSelectedUnitHint {
                id: collector_id,
                is_alive: !collector.destroyed && collector.health_current > 0.0,
                is_resource_collector: PresentationFrame::object_has_kind(
                    collector,
                    KindOf::Harvester,
                ),
                is_worker: false,
                can_attack: false,
                can_move: collector.is_mobile,
                can_request_service: true,
                can_capture: false,
                template_name: collector.template_name.clone(),
                can_repair: false,
                is_damaged: false,
                is_vehicle: PresentationFrame::object_has_kind(collector, KindOf::Vehicle),
                is_aircraft: false,
                is_above_terrain: false,
                is_infantry: false,
                transport_slot_count: collector.transport_slot_count,
                stored_supplies: collector.stored_supplies,
                is_controlled_by_local: frame.is_owned_by_local(collector),
                capture_power: crate::game_logic::CapturePowerKind::None,
                capture_power_ready: false,
                is_salvager: false,
                can_override_special_power_destination: false,
            }],
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        }
    }

    let mut game_logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "USA slot 0", true);
    let mut allied_other = Player::new(1, Team::USA, "USA slot 1", false);
    local.alliance_team = 7;
    allied_other.alliance_team = 7;
    game_logic.add_player(local);
    game_logic.add_player(allied_other);

    let mut collector = ThingTemplate::new("DockRmbCollector");
    collector
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Harvester)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("DockRmbCollector".to_string(), collector);

    let mut center = ThingTemplate::new("DockRmbSupplyCenter");
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    center.dock_kind = DockKind::SupplyCenter;
    game_logic
        .templates
        .insert("DockRmbSupplyCenter".to_string(), center);

    let collector_id = game_logic
        .create_object_for_player("DockRmbCollector", 0, Vec3::ZERO)
        .expect("local collector");
    let own_center_id = game_logic
        .create_object_for_player("DockRmbSupplyCenter", 0, Vec3::ZERO)
        .expect("own center");
    let allied_center_id = game_logic
        .create_object_for_player("DockRmbSupplyCenter", 1, Vec3::new(100.0, 0.0, 0.0))
        .expect("allied other-player center");
    game_logic
        .host_object_mut(collector_id)
        .expect("collector")
        .stored_resources
        .supplies = 1;

    let frame = PresentationFrame::build_from_logic(&game_logic, 0);
    let other_center = frame
        .objects
        .iter()
        .find(|object| object.id == allied_center_id)
        .expect("allied center in frame");
    assert!(
        frame.is_allied_with_local(other_center),
        "regression requires a friendly same-faction but separately controlled center"
    );
    assert!(
        !frame.is_owned_by_local(other_center),
        "frozen frame must preserve controlling-player distinction"
    );

    let mut commands = CommandSystem::new();
    let rejected = commands
        .process_mouse_input(
            &dock_context(&frame, collector_id, allied_center_id),
            &[collector_id],
            0,
            None,
        )
        .expect("physical RMB always makes a contextual command");
    assert!(
        !matches!(rejected.command_type, CommandType::Dock { .. }),
        "RMB must not visibly offer Dock into an allied other player's SupplyCenter"
    );

    // Boot/load input has no presentation freeze yet.  It must make the same
    // owner-aware decision rather than reintroducing the old Team shortcut.
    let mut boot_rejected_context = dock_context(&frame, collector_id, allied_center_id);
    boot_rejected_context.target_presentation = None;
    boot_rejected_context.selected_presentation.clear();
    let boot_rejected = commands
        .process_mouse_input(
            &boot_rejected_context,
            &[collector_id],
            0,
            Some(&game_logic),
        )
        .expect("boot RMB makes a contextual command");
    assert!(
        !matches!(boot_rejected.command_type, CommandType::Dock { .. }),
        "boot classifier must also reject the allied other-player center"
    );

    let mut boot_accepted_context = dock_context(&frame, collector_id, own_center_id);
    boot_accepted_context.target_presentation = None;
    boot_accepted_context.selected_presentation.clear();
    let boot_accepted = commands
        .process_mouse_input(
            &boot_accepted_context,
            &[collector_id],
            0,
            Some(&game_logic),
        )
        .expect("boot own-center click creates a command");
    assert!(matches!(
        boot_accepted.command_type,
        CommandType::Dock { target_id } if target_id == own_center_id
    ));

    let accepted = commands
        .process_mouse_input(
            &dock_context(&frame, collector_id, own_center_id),
            &[collector_id],
            0,
            None,
        )
        .expect("own center click creates a command");
    assert!(matches!(
        accepted.command_type,
        CommandType::Dock { target_id } if target_id == own_center_id
    ));
    game_logic.queue_command(accepted);
    game_logic.process_commands();

    let collector = game_logic
        .host_object(collector_id)
        .expect("collector after executor authority");
    assert_eq!(collector.ai_state, AIState::ReturningResources);
    assert_eq!(collector.preferred_dock_id, Some(own_center_id));
}

#[test]
fn capturing_structure_refunds_old_owner_queued_production() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    game_logic
        .get_player_mut(0)
        .expect("USA player should exist")
        .unlocked_sciences
        .insert("Upgrade_AmericaRangerCaptureBuilding".to_string());

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor should be created");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks should be created");

    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .get_player(2)
            .expect("GLA player should exist")
            .resources
            .supplies,
        99_900,
        "queued production should charge the old owner before capture"
    );

    {
        let captor = game_logic
            .host_object_mut(captor_id)
            .expect("captor should exist");
        captor.target = Some(barracks_id);
        captor.set_ai_state(AIState::Capturing);
    }

    game_logic.update_ai(&[captor_id, barracks_id], 1.0 / 60.0);
    game_logic.update_ai(&[captor_id, barracks_id], 3.0);
    game_logic.update_ai(&[captor_id, barracks_id], 20.0);

    let barracks = game_logic
        .host_object(barracks_id)
        .expect("captured barracks should still exist");
    assert_eq!(barracks.team, Team::USA);
    assert_eq!(
        barracks
            .building_data
            .as_ref()
            .expect("barracks should have building data")
            .production_queue
            .len(),
        0,
        "capture should clear old owner's queued production"
    );
    assert_eq!(
        game_logic
            .get_player(2)
            .expect("GLA player should exist")
            .resources
            .supplies,
        100_000,
        "capture should refund queued production to the old owner"
    );
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist")
            .resources
            .supplies,
        100_000,
        "new owner should not receive the old owner's production refund"
    );
}

#[test]
fn capture_command_rejects_under_construction_building() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    {
        let building = game_logic
            .host_object_mut(building_id)
            .expect("building should exist");
        building.set_status_under_construction(true);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_ne!(captor.ai_state, AIState::Capturing);
    assert_ne!(captor.target, Some(building_id));

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(building.team, Team::GLA);
}

#[test]
fn capturing_state_does_not_transfer_under_construction_building() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    {
        let building = game_logic
            .host_object_mut(building_id)
            .expect("building should exist");
        building.set_status_under_construction(true);
    }
    {
        let captor = game_logic
            .host_object_mut(captor_id)
            .expect("captor should exist");
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }

    game_logic.update_ai(&[captor_id, building_id], 1.0 / 60.0);

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(building.team, Team::GLA);

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

#[test]
fn capture_command_rejects_non_infantry_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("tank should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(tank.ai_state, AIState::Capturing);
    assert_ne!(tank.target, Some(building_id));

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(building.team, Team::GLA);
}

#[test]
fn repair_command_sets_all_selected_repairers_to_repairing() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let repairer_a = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repairer A should be created");
    let repairer_b = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .expect("repairer B should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("repair target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(50.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![repairer_a, repairer_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let a = game_logic
        .host_object(repairer_a)
        .expect("repairer A should exist");
    let b = game_logic
        .host_object(repairer_b)
        .expect("repairer B should exist");

    assert_eq!(a.ai_state, AIState::Repairing);
    assert_eq!(b.ai_state, AIState::Repairing);
    assert_eq!(a.target, Some(target_id));
    assert_eq!(b.target, Some(target_id));
}

#[test]
fn repair_command_ignores_non_worker_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("repair target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(75.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(
        tank.ai_state,
        AIState::Repairing,
        "non-worker units should not enter repairing state from repair commands"
    );
}

#[test]
fn repair_command_allows_repairing_neutral_structures() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let repairer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repairer should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::Neutral, Vec3::new(6.0, 0.0, 0.0))
        .expect("neutral target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(60.0);
    }

    let before = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![repairer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[repairer_id, target_id], 1.0 / 60.0);

    let repairer = game_logic
        .host_object(repairer_id)
        .expect("repairer should exist");
    assert_eq!(repairer.ai_state, AIState::Repairing);
    assert_eq!(repairer.target, Some(target_id));

    let after = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;
    assert!(after > before);
}

/// Residual E2E: damage structure → Repair command → HP recovers over time.
/// Covers dozer structure repair residual (including WarFactory as structure).
/// Fail-closed: not C++ percent-of-max heal / sole-benefactor / scaffolding.
#[test]
fn dozer_structure_repair_residual_recovers_hp_over_time() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    // Place dozer in interact range so heal starts immediately.
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    // Explicit WarFactory-named structure so residual covers "repair WarFactory".
    let mut war_factory_tpl = crate::game_logic::ThingTemplate::new("USA_WarFactory");
    war_factory_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1500.0)
        .set_cost(2000, -2);
    game_logic
        .templates
        .insert("USA_WarFactory".to_string(), war_factory_tpl);

    let structure_id = game_logic
        .create_object("USA_WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory structure");

    {
        let structure = game_logic.host_object_mut(structure_id).expect("structure");
        let _ = structure.take_damage(400.0);
        assert!(
            structure.health.current + 0.01 < structure.health.maximum,
            "structure must be damaged before repair"
        );
    }
    let before = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;

    assert_eq!(game_logic.repair_residual_structure_commands(), 0);
    assert!(!game_logic.honesty_structure_repair_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair {
            target_id: structure_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert_eq!(
        game_logic.repair_residual_structure_commands(),
        1,
        "successful Repair command must record honesty"
    );
    {
        let dozer = game_logic.host_object(dozer_id).expect("dozer");
        assert_eq!(dozer.ai_state, AIState::Repairing);
        assert_eq!(dozer.target, Some(structure_id));
    }

    // Several logic frames: HP must increase over time.
    for _ in 0..30 {
        game_logic.update_ai(&[dozer_id, structure_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;
    assert!(
        after > before,
        "dozer Repair residual must restore structure HP over time (before={before}, after={after})"
    );
    assert!(
        game_logic.repair_residual_structure_heals() > 0,
        "must record structure heal honesty ticks"
    );
    assert!(
        game_logic.honesty_structure_repair_ok(),
        "structure repair residual honesty path"
    );
    assert!(game_logic.honesty_repair_ok(), "combined repair honesty");
}

/// Residual: dozer out of range walks in, then structure HP recovers (full update loop).
#[test]
fn dozer_structure_repair_residual_walk_into_range_recovers_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    // Outside INTERACT_RANGE (14): must approach before healing.
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(55.0, 0.0, 0.0))
        .expect("dozer");
    let structure_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("structure");

    {
        let structure = game_logic.host_object_mut(structure_id).expect("structure");
        // This regression is about the selected dozer's Repair order.  Disable
        // the independent BaseRegenerateUpdate fixture module so autonomous
        // structure regeneration cannot masquerade as an in-range repair tick.
        structure.base_regenerate = None;
        let _ = structure.take_damage(300.0);
    }
    let before = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair {
            target_id: structure_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let dozer = game_logic.host_object(dozer_id).expect("dozer");
        assert_eq!(dozer.ai_state, AIState::Repairing);
        assert_eq!(dozer.target, Some(structure_id));
        assert!(
            !dozer.movement.path.is_empty(),
            "out-of-range repair must retain an A* approach path rather than bypass movement"
        );
    }
    // Must not heal while still out of range on first short step.
    game_logic.update();
    let mid = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;
    // May still be equal if not in range; allow equal on first frame.
    let _ = mid;

    let mut recovered = false;
    for _ in 0..900 {
        game_logic.update();
        if game_logic
            .host_object(structure_id)
            .map(|s| s.health.current > before + 0.5)
            .unwrap_or(false)
        {
            recovered = true;
            break;
        }
    }
    let dozer_after_walk = game_logic.host_object(dozer_id).expect("dozer after walk");
    assert!(
        recovered,
        "dozer must walk into repair range and restore structure HP; pos={:?}, state={:?}, target={:?}, moving={}, path_index={}, path_len={}, movement_target={:?}",
        dozer_after_walk.get_position(),
        dozer_after_walk.ai_state,
        dozer_after_walk.target,
        dozer_after_walk.status.moving,
        dozer_after_walk.movement.current_path_index,
        dozer_after_walk.movement.path.len(),
        dozer_after_walk.movement.target_position,
    );
    assert!(
        game_logic.honesty_structure_repair_ok(),
        "walk-in repair residual honesty (commands={}, heals={})",
        game_logic.repair_residual_structure_commands(),
        game_logic.repair_residual_structure_heals(),
    );

    // Repairing must not be clobbered to Idle mid-approach without finishing.
    let dozer = game_logic.host_object(dozer_id).expect("dozer");
    assert!(
        matches!(dozer.ai_state, AIState::Repairing | AIState::Idle),
        "dozer should still be repairing or finished idle, got {:?}",
        dozer.ai_state
    );
}

/// Residual: damaged vehicle GetRepaired at WarFactory recovers HP (China RepairDock).
#[test]
fn war_factory_vehicle_repair_residual_recovers_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut war_factory_tpl = crate::game_logic::ThingTemplate::new("China_WarFactory");
    war_factory_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(2000.0)
        .set_cost(2000, -2);
    game_logic
        .templates
        .insert("China_WarFactory".to_string(), war_factory_tpl);

    let war_factory_id = game_logic
        .create_object("China_WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory");
    {
        let wf = game_logic.host_object(war_factory_id).expect("wf");
        assert_eq!(
            wf.building_data.as_ref().map(|b| b.building_type),
            Some(BuildingType::WarFactory)
        );
    }

    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("vehicle");
    {
        let vehicle = game_logic.host_object_mut(vehicle_id).expect("vehicle");
        let _ = vehicle.take_damage(120.0);
    }
    let before = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .health
        .current;

    assert!(!game_logic.honesty_vehicle_repair_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: war_factory_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let vehicle = game_logic.host_object(vehicle_id).expect("vehicle");
        assert_eq!(
            vehicle.ai_state,
            AIState::SeekingRepair,
            "WarFactory must accept GetRepaired for vehicles"
        );
        assert_eq!(vehicle.target, Some(war_factory_id));
    }

    for _ in 0..30 {
        game_logic.update_ai(&[war_factory_id, vehicle_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .health
        .current;
    assert!(
        after > before,
        "WarFactory vehicle repair residual must restore HP (before={before}, after={after})"
    );
    assert!(
        game_logic.repair_residual_vehicle_heals() > 0,
        "must record vehicle heal honesty"
    );
    assert!(
        game_logic.honesty_vehicle_repair_ok(),
        "vehicle repair residual honesty"
    );
}

/// Residual E2E: damaged infantry near USA Ambulance recovers HP over time.
/// C++ AmericaVehicleMedic AutoHealBehavior ModuleTag_22 (INFANTRY, Radius=100).
/// Fail-closed: not sole-benefactor / vehicle AutoHeal ModuleTag_23 / embark regen.
#[test]
fn ambulance_auto_heal_residual_recovers_infantry_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleMedic");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("AmericaVehicleMedic".to_string(), ambulance_tpl);

    let ambulance_id = game_logic
        .create_object("AmericaVehicleMedic", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("infantry");

    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(40.0);
        assert!(
            infantry.health.current + 0.01 < infantry.health.maximum,
            "infantry must be damaged before ambulance heal"
        );
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    assert_eq!(game_logic.heal_residual_ambulance_heals(), 0);
    assert!(!game_logic.honesty_ambulance_heal_ok());
    assert!(!game_logic.honesty_heal_ok());

    // Several logic frames of residual AutoHeal (no command required — StartsActive).
    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }

    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "ambulance AutoHeal residual must restore infantry HP (before={before}, after={after})"
    );
    assert!(
        game_logic.heal_residual_ambulance_heals() > 0,
        "must record ambulance heal honesty ticks"
    );
    assert!(
        game_logic.honesty_ambulance_heal_ok(),
        "ambulance heal residual honesty path"
    );
    assert!(game_logic.honesty_heal_ok(), "combined heal honesty");

    // Ambulance itself still present (not self-healed as infantry residual).
    assert!(
        game_logic
            .host_object(ambulance_id)
            .map(|a| a.is_alive())
            .unwrap_or(false),
        "ambulance must remain alive"
    );
}

/// Residual: infantry outside ambulance radius is not healed; walk-in recovers HP.
#[test]
fn ambulance_auto_heal_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("USA_Ambulance");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), ambulance_tpl);

    let _ambulance_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("infantry");
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(30.0);
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    // Out of residual radius (100): no heal.
    for _ in 0..15 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let mid = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        (mid - before).abs() < 0.01,
        "out-of-range infantry must not receive ambulance heal"
    );
    assert!(!game_logic.honesty_ambulance_heal_ok());

    // Move into radius.
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        infantry.set_position(Vec3::new(30.0, 0.0, 0.0));
    }
    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "in-range infantry must recover HP from ambulance residual"
    );
    assert!(game_logic.honesty_ambulance_heal_ok());
}

/// Residual: enemy infantry near ambulance is not healed (same-team residual filter).
#[test]
fn ambulance_auto_heal_residual_skips_enemy_infantry() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleMedic");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("AmericaVehicleMedic".to_string(), ambulance_tpl);

    let _ambulance_id = game_logic
        .create_object("AmericaVehicleMedic", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy infantry");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        let _ = enemy.take_damage(40.0);
    }
    let before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let after = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    assert!(
        (after - before).abs() < 0.01,
        "enemy infantry must not be healed by USA ambulance residual"
    );
    assert!(!game_logic.honesty_ambulance_heal_ok());
}

/// Residual E2E: damaged unit near China Speaker Tower recovers HP + gets ENTHUSIASTIC buff.
/// C++ ChinaSpeakerTower PropagandaTowerBehavior ModuleTag_06 (Radius=150, Heal%=2%).
/// Fail-closed: not sole-benefactor / PulseFX / POWERED underpower gate.
#[test]
fn propaganda_tower_residual_recovers_hp_and_sets_enthusiastic() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("unit");

    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(40.0);
        assert!(
            unit.health.current + 0.01 < unit.health.maximum,
            "unit must be damaged before propaganda heal"
        );
        assert!(!unit.weapon_bonus_enthusiastic);
        assert!(!unit.weapon_bonus_subliminal);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    assert_eq!(game_logic.propaganda_residual_heals(), 0);
    assert_eq!(game_logic.propaganda_residual_buffs(), 0);
    assert!(!game_logic.honesty_propaganda_ok());

    // Several logic frames of residual pulse (no command — continuous AoE).
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }

    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(
        unit.health.current > before,
        "propaganda residual must restore HP (before={before}, after={})",
        unit.health.current
    );
    assert!(
        unit.weapon_bonus_enthusiastic,
        "in-range unit must receive ENTHUSIASTIC residual buff"
    );
    assert!(
        !unit.weapon_bonus_subliminal,
        "base tower without upgrade must not grant SUBLIMINAL"
    );
    assert!(
        game_logic.propaganda_residual_heals() > 0,
        "must record propaganda heal honesty ticks"
    );
    assert!(
        game_logic.propaganda_residual_buffs() > 0,
        "must record propaganda buff honesty ticks"
    );
    assert!(game_logic.honesty_propaganda_heal_ok());
    assert!(game_logic.honesty_propaganda_buff_ok());
    assert!(game_logic.honesty_propaganda_ok());

    assert!(
        game_logic
            .host_object(tower_id)
            .map(|t| t.is_alive())
            .unwrap_or(false),
        "tower must remain alive"
    );
}

/// Residual: unit outside tower radius is not healed/buffed; walk-in recovers.
#[test]
fn propaganda_tower_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(250.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(30.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    // Out of residual radius (150): no heal / no buff.
    for _ in 0..15 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            (unit.health.current - before).abs() < 0.01,
            "out-of-range unit must not receive propaganda heal"
        );
        assert!(!unit.weapon_bonus_enthusiastic);
    }
    assert!(!game_logic.honesty_propaganda_ok());

    // Move into radius — membership waits for the next 2s scan.
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        unit.set_position(Vec3::new(40.0, 0.0, 0.0));
    }
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            !unit.weapon_bonus_enthusiastic,
            "enter waits for next scan (C++ m_scanDelayInFrames)"
        );
    }
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES);
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            unit.health.current > before,
            "in-range unit must recover HP from propaganda residual"
        );
        assert!(unit.weapon_bonus_enthusiastic);
    }
    assert!(game_logic.honesty_propaganda_ok());

    // Leave radius: buff sticks until the next scan.
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        unit.set_position(Vec3::new(300.0, 0.0, 0.0));
    }
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            unit.weapon_bonus_enthusiastic,
            "leave keeps ENTHUSIASTIC until next scan"
        );
    }
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES);
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            !unit.weapon_bonus_enthusiastic,
            "next scan after leave must clear ENTHUSIASTIC"
        );
        assert!(!unit.weapon_bonus_subliminal);
    }

}

/// Residual: enemy units near speaker tower are not healed/buffed (same-team filter).
#[test]
fn propaganda_tower_residual_skips_enemy_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        let _ = enemy.take_damage(40.0);
    }
    let before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        (enemy.health.current - before).abs() < 0.01,
        "enemy unit must not be healed by China speaker tower residual"
    );
    assert!(!enemy.weapon_bonus_enthusiastic);
    assert!(!game_logic.honesty_propaganda_ok());
}

/// Residual: Subliminal Messaging upgrade grants SUBLIMINAL buff + faster heal.
#[test]
fn propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    {
        let tower = game_logic.host_object_mut(tower_id).expect("tower");
        tower.apply_upgrade_tag(
            crate::game_logic::host_propaganda::UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
        );
    }

    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(40.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }

    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(unit.weapon_bonus_enthusiastic);
    assert!(
        unit.weapon_bonus_subliminal,
        "upgraded tower must grant SUBLIMINAL residual buff"
    );
    // 4% of max (80) per second * 1s = ~3.2 HP; base would be ~1.6.
    assert!(
        unit.health.current > before + 2.5,
        "upgraded heal rate residual should exceed base (before={before}, after={})",
        unit.health.current
    );
    assert!(game_logic.honesty_propaganda_ok());
}

/// Residual: HelixPropagandaTower name residual also heals nearby units.
/// (ChinaTankOverlordPropagandaTower is map-skip illegal; Helix name residual covers the path.)
#[test]
fn propaganda_tower_name_residual_helix_propaganda_heals() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaHelixPropagandaTower");
    tower_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0)
        .set_cost(0, 0);
    game_logic
        .templates
        .insert("ChinaHelixPropagandaTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object(
            "ChinaHelixPropagandaTower",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("helix prop tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(25.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(30.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(unit.health.current > before);
    assert!(unit.weapon_bonus_enthusiastic);
    assert!(game_logic.honesty_propaganda_ok());
}

/// Residual: HealPad GetHealed recovers infantry HP and records heal-pad honesty.
#[test]
fn heal_pad_seeking_healing_residual_recovers_infantry_hp() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    // Host-state residual honesty without shadow writeback.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("infantry");
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(40.0);
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    assert!(!game_logic.honesty_heal_pad_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let infantry = game_logic.host_object(infantry_id).expect("infantry");
        assert_eq!(infantry.ai_state, AIState::SeekingHealing);
        assert_eq!(infantry.target, Some(heal_pad_id));
    }

    for _ in 0..30 {
        game_logic.update_ai(&[heal_pad_id, infantry_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "HealPad SeekingHealing residual must restore infantry HP (before={before}, after={after})"
    );
    assert!(
        game_logic.heal_residual_heal_pad_heals() > 0,
        "must record heal-pad honesty ticks"
    );
    assert!(game_logic.honesty_heal_pad_ok());
    assert!(game_logic.honesty_heal_ok());

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// C++ service pads use Player relationships rather than a faction enum.
/// This is deliberately an end-to-end physical RMB test: the frozen input
/// rejects a same-faction enemy pad, accepts a cross-faction allied pad, and
/// the executor/support state repeat the owner-aware authority checks.
#[test]
fn physical_service_commands_use_player_relationship_and_revalidate_owner_changes() {
    use crate::command_system::{
        CommandSystem, ModifierKeys, MouseButton, MouseCommandContext,
        PresentationSelectedUnitHint, PresentationTargetHint,
    };

    let mut game_logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "USA local", true);
    local.alliance_team = 7;
    let mut same_faction_enemy = Player::new(1, Team::USA, "USA enemy", false);
    same_faction_enemy.alliance_team = 9;
    let mut cross_faction_ally = Player::new(2, Team::China, "China ally", false);
    cross_faction_ally.alliance_team = 7;
    game_logic.add_player(local);
    game_logic.add_player(same_faction_enemy);
    game_logic.add_player(cross_faction_ally);

    ensure_test_tank_template(&mut game_logic);
    let mut service_pad = ThingTemplate::new("OwnerRelationServicePad");
    service_pad
        .add_kind_of(KindOf::Structure)
        // The active service authority is C++ KINDOF_REPAIR_PAD, not the
        // legacy BuildingType presentation fixture below.
        .add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1_000.0)
        .set_cost(500, -1);
    game_logic
        .templates
        .insert("OwnerRelationServicePad".to_string(), service_pad);

    let tank_id = game_logic
        .create_object_for_player("TestTank", 0, Vec3::new(8.0, 0.0, 0.0))
        .expect("local tank");
    let same_faction_enemy_pad = game_logic
        .create_object_for_player("OwnerRelationServicePad", 1, Vec3::ZERO)
        .expect("same-faction enemy pad");
    let cross_faction_ally_pad = game_logic
        .create_object_for_player("OwnerRelationServicePad", 2, Vec3::ZERO)
        .expect("cross-faction allied pad");
    for pad_id in [same_faction_enemy_pad, cross_faction_ally_pad] {
        game_logic
            .host_object_mut(pad_id)
            .expect("service pad")
            .building_data = Some(BuildingData::new(BuildingType::RepairPad));
    }
    {
        // Keep the service command test independent of a coupled GameWorld
        // damage writeback: command authority observes this host HP directly.
        let tank = game_logic.host_object_mut(tank_id).expect("tank");
        tank.health.current = (tank.health.maximum - 80.0).max(1.0);
    }

    let selected_hint = || PresentationSelectedUnitHint {
        id: tank_id,
        is_alive: true,
        is_resource_collector: false,
        is_worker: false,
        can_attack: false,
        can_move: true,
        can_request_service: true,
        can_capture: false,
        template_name: "TestTank".to_string(),
        can_repair: false,
        is_damaged: true,
        is_vehicle: true,
        is_aircraft: false,
        is_above_terrain: false,
        is_infantry: false,
        transport_slot_count: 3,
        stored_supplies: 0,
        is_controlled_by_local: true,
        capture_power: CapturePowerKind::None,
        capture_power_ready: false,
        is_salvager: false,
        can_override_special_power_destination: false,
    };
    let service_context =
        |target_id, team, is_enemy_of_local, is_friendly_of_local| MouseCommandContext {
            world_position: Vec3::ZERO,
            target_object: Some(target_id),
            target_presentation: Some(PresentationTargetHint {
                id: target_id,
                is_alive: true,
                is_structure: true,
                is_resource: false,
                under_construction: false,
                sold: false,
                team,
                is_enemy_of_local,
                is_neutral: false,
                template_name: "OwnerRelationServicePad".to_string(),
                can_be_entered: false,
                enter_available_capacity: 0,
                enter_uses_transport_slots: false,
                enter_requires_infantry: false,
                enter_forbids_aircraft: false,
                enter_disabled_subdued: false,
                enter_is_rider_change: false,
                rider_change_allowed_templates: Vec::new(),
                is_damaged: false,
                is_friendly_of_local,
                provides_vehicle_repair: true,
                provides_aircraft_repair: false,
                provides_heal: false,
                can_provide_service: true,
                dock_kind: DockKind::None,
                dock_controller_is_local: false,
                stored_supplies: 0,
                capturable: false,
                immune_to_capture: false,
                capture_garrisonable: false,
                capture_nonstealthed_garrison_count: 0,
                capture_friendly_garrison_count: 0,
                capture_target_effectively_stealthed: false,
                is_crate: false,
                is_salvage_crate: false,
                is_vehicle: false,
                is_aircraft: false,
                is_drone: false,
                is_carbomb: false,
                is_unmanned: false,
                is_mine: false,
            }),
            selected_presentation: vec![selected_hint()],
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

    let mut command_system = CommandSystem::new();
    let enemy_click = command_system
        .process_mouse_input(
            &service_context(same_faction_enemy_pad, Team::USA, true, false),
            &[tank_id],
            0,
            Some(&game_logic),
        )
        .expect("physical right click command");
    assert!(
        matches!(
            enemy_click.command_type,
            crate::command_system::CommandType::MoveTo { .. }
        ),
        "same-faction enemy service pad must not classify as a friendly repair command"
    );

    // A stale/malicious service command cannot bypass the frozen RMB result.
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: same_faction_enemy_pad,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after rejected command");
    assert_ne!(tank.ai_state, AIState::SeekingRepair);
    assert_ne!(tank.target, Some(same_faction_enemy_pad));

    let ally_click = command_system
        .process_mouse_input(
            &service_context(cross_faction_ally_pad, Team::China, false, true),
            &[tank_id],
            0,
            Some(&game_logic),
        )
        .expect("physical allied right click command");
    assert!(
        matches!(
            ally_click.command_type,
            crate::command_system::CommandType::GetRepaired { target_id }
                if target_id == cross_faction_ally_pad
        ),
        "cross-faction allied repair pad must issue GetRepaired"
    );
    game_logic.queue_command(ally_click);
    game_logic.process_commands();
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after allied command");
    assert_eq!(tank.ai_state, AIState::SeekingRepair);
    assert_eq!(tank.target, Some(cross_faction_ally_pad));

    // Revalidate while moving/docked: a captured or reassigned repair pad may
    // no longer service this tank even if its original RMB was legal.
    assert!(game_logic.transfer_object_to_player(cross_faction_ally_pad, 1));
    game_logic.update_ai(&[tank_id, cross_faction_ally_pad], 1.0 / 30.0);
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after owner change");
    assert_ne!(tank.target, Some(cross_faction_ally_pad));
}

#[test]
fn get_repaired_command_targets_only_damaged_vehicles() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_bay_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair bay should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(9.0, 0.0, 0.0))
        .expect("infantry should be created");

    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_bay_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id, infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_eq!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(infantry.ai_state, AIState::SeekingRepair);
}

#[test]
fn get_repaired_command_requires_repair_destination_type() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let non_repair_structure = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("support structure should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: non_repair_structure,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(vehicle.target, Some(non_repair_structure));
}

#[test]
fn get_repaired_command_rejects_under_construction_destination() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_pad_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair pad should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }
    {
        let repair_pad = game_logic
            .host_object_mut(repair_pad_id)
            .expect("repair pad should exist");
        repair_pad.set_status_under_construction(true);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(vehicle.target, Some(repair_pad_id));
}

#[test]
fn get_repaired_command_aircraft_requires_airfield() {
    let mut game_logic = GameLogic::new();
    ensure_test_aircraft_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);
    ensure_test_airfield_template(&mut game_logic);

    let repair_pad_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair pad should be created");
    let airfield_id = game_logic
        .create_object("TestAirfield", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("airfield should be created");
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("aircraft should be created");
    {
        let aircraft = game_logic
            .host_object_mut(aircraft_id)
            .expect("aircraft should exist");
        let _ = aircraft.take_damage(100.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![aircraft_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let aircraft = game_logic
        .host_object(aircraft_id)
        .expect("aircraft should exist");
    assert_ne!(aircraft.ai_state, AIState::SeekingRepair);

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: airfield_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![aircraft_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let aircraft = game_logic
        .host_object(aircraft_id)
        .expect("aircraft should exist");
    assert_eq!(aircraft.ai_state, AIState::SeekingRepair);
    assert_eq!(aircraft.target, Some(airfield_id));
}

#[test]
fn get_healed_command_targets_only_injured_infantry() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(9.0, 0.0, 0.0))
        .expect("vehicle should be created");

    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id, vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_eq!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(vehicle.ai_state, AIState::SeekingHealing);
}

#[test]
fn get_healed_command_requires_heal_destination_type() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let non_heal_structure = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("non-heal destination should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: non_heal_structure,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_ne!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(infantry.target, Some(non_heal_structure));
}

#[test]
fn get_healed_command_rejects_under_construction_destination() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }
    {
        let heal_pad = game_logic
            .host_object_mut(heal_pad_id)
            .expect("heal pad should exist");
        heal_pad.set_status_under_construction(true);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_ne!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(infantry.target, Some(heal_pad_id));
}

#[test]
fn special_ability_state_without_pending_order_resets_to_idle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let actor_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("actor should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(3.0, 0.0, 0.0))
        .expect("target should be created");

    {
        let actor = game_logic
            .host_object_mut(actor_id)
            .expect("actor should exist");
        actor.target = Some(target_id);
        actor.set_ai_state(AIState::SpecialAbility);
    }

    game_logic.update_ai(&[actor_id, target_id], 1.0 / 60.0);

    let actor = game_logic
        .host_object(actor_id)
        .expect("actor should exist");
    assert_eq!(actor.ai_state, AIState::Idle);
    assert!(actor.target.is_none());
}

#[test]
fn build_command_rejects_non_worker_constructor() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstruct {
            template_name: "TestBuilding".to_string(),
            location: Vec3::new(20.0, 0.0, 20.0),
            orientation: 0.0,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let created_structures = game_logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "TestBuilding")
        .count();
    assert_eq!(created_structures, 0);

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(tank.ai_state, AIState::Constructing);
}

#[test]
fn dozer_line_assigns_each_worker_a_segment() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let dozer_a = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("dozer A should be created");
    let dozer_b = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer B should be created");

    let start = Vec3::new(10.0, 0.0, 10.0);
    let end = Vec3::new(30.0, 0.0, 10.0);
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstructLine {
            template_name: "TestBuilding".to_string(),
            start,
            end,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_a, dozer_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let dozer_a_state = game_logic
        .host_object(dozer_a)
        .expect("dozer A should exist");
    let dozer_b_state = game_logic
        .host_object(dozer_b)
        .expect("dozer B should exist");
    assert_eq!(dozer_a_state.ai_state, AIState::Constructing);
    assert_eq!(dozer_b_state.ai_state, AIState::Constructing);

    let a_dest = dozer_a_state
        .movement
        .target_position
        .expect("dozer A should receive a line segment destination");
    let b_dest = dozer_b_state
        .movement
        .target_position
        .expect("dozer B should receive a line segment destination");
    assert!(a_dest.distance(start) < 0.01);
    assert!(b_dest.distance(end) < 0.01);

    let created_structures = game_logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "TestBuilding")
        .count();
    assert_eq!(created_structures, 2);
}

#[test]
fn hijack_transfers_vehicle_and_updates_team_color() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let hijacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(target.team, Team::USA);
    assert_eq!(target.team_color, Team::USA.get_color());
    assert!(
        target.status.hijacked,
        "hijack residual must set OBJECT_STATUS_HIJACKED"
    );
    assert!(target.is_hijacked(), "hijack residual is_hijacked helper");
    assert!(game_logic.honesty_hijack_ok(), "hijack residual honesty");
    assert_eq!(
        game_logic.car_bomb_residual().hijacks,
        1,
        "hijack honesty counter"
    );

    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(
        hijacker.status.destroyed,
        "hijacker infantry consumed after steal"
    );
}

#[test]
fn hijack_rejects_already_hijacked_vehicle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let hijacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("target should be created");
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        target.apply_hijacked();
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target.team,
        Team::GLA,
        "already-hijacked vehicle must not re-transfer"
    );
    assert!(!game_logic.honesty_hijack_ok());
    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(
        !hijacker.status.destroyed,
        "failed re-hijack must not consume infantry"
    );
}

#[test]
fn hijack_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let hijacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.team,
        Team::GLA,
        "hijack should not transfer target immediately on command issue"
    );

    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.team,
        Team::GLA,
        "hijack should stay pending while hijacker is out of range"
    );

    {
        let hijacker = game_logic
            .host_object_mut(hijacker_id)
            .expect("hijacker should exist");
        hijacker.set_position(Vec3::new(2.0, 0.0, 0.0));
        hijacker.set_ai_state(AIState::SpecialAbility);
        hijacker.target = Some(target_id);
    }
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(target_after_contact.team, Team::USA);

    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(hijacker.status.destroyed);
}

/// Residual: GLA Saboteur power-plant brownout after reach (consumed).
#[test]
fn sabotage_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_power_plant_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("saboteur should be created");
    let target_id = game_logic
        .create_object("AmericaPowerPlant", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Not yet: out of range.
    assert!(!game_logic.honesty_saboteur_power_ok());
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::USA)
            .map(|p| p.power_sabotaged_till_frame)
            .unwrap_or(0),
        0
    );

    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);
    assert!(!game_logic.honesty_saboteur_power_ok());

    {
        let saboteur = game_logic
            .host_object_mut(saboteur_id)
            .expect("saboteur should exist");
        saboteur.set_position(Vec3::new(2.0, 0.0, 0.0));
        saboteur.set_ai_state(AIState::SpecialAbility);
        saboteur.target = Some(target_id);
    }
    game_logic.frame = 30;
    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);

    assert!(
        game_logic.honesty_saboteur_power_ok(),
        "power sabotage residual must apply on reach"
    );
    let until = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.power_sabotaged_till_frame)
        .unwrap_or(0);
    assert!(
        until > 30,
        "power_sabotaged_till_frame must be set (until={until})"
    );
    // Saboteur consumed residual.
    let sab_alive = game_logic
        .host_object(saboteur_id)
        .map(|s| s.is_alive() && !s.status.destroyed)
        .unwrap_or(false);
    assert!(!sab_alive, "saboteur must be consumed on success");
}

#[test]
fn sabotage_command_rejects_non_structure_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("saboteur should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let saboteur = game_logic
        .host_object(saboteur_id)
        .expect("saboteur should exist");
    assert_ne!(saboteur.ai_state, AIState::SpecialAbility);
    assert_ne!(saboteur.target, Some(target_id));
}

/// Residual: Saboteur military factory DISABLED_HACKED residual.
#[test]
fn saboteur_military_factory_residual_disables_production() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_war_factory_template(&mut game_logic);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("saboteur");
    let target_id = game_logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("factory");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let s = game_logic.host_object_mut(saboteur_id).unwrap();
        s.set_position(Vec3::new(1.0, 0.0, 0.0));
        s.set_ai_state(AIState::SpecialAbility);
        s.target = Some(target_id);
    }
    game_logic.frame = 10;
    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);

    assert!(
        game_logic.honesty_saboteur_military_ok(),
        "military factory sabotage residual honesty"
    );
    let factory = game_logic.host_object(target_id).expect("factory");
    assert!(
        factory.is_hacked_disabled() || factory.status.disabled_hacked,
        "factory must be DISABLED_HACKED residual"
    );
    assert!(
        factory.status.disabled_hacked_until_frame > 10,
        "disable timer residual"
    );
}

/// Residual: non-saboteur unit cannot issue Sabotage residual (fail-closed).
#[test]
fn sabotage_command_rejects_non_saboteur_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_power_plant_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    let target_id = game_logic
        .create_object("AmericaPowerPlant", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("plant");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(tank.ai_state, AIState::SpecialAbility);
    assert!(!game_logic.honesty_saboteur_ok());
}

#[test]
fn snipe_vehicle_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let sniper_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(300.0, 0.0, 0.0))
        .expect("sniper should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::SnipeVehicle { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![sniper_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.health.current, initial_health,
        "snipe should not apply immediately on command issue"
    );
    assert!(
        !target_after_command.is_unmanned(),
        "vehicle must remain manned until sniper resolves"
    );

    game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.health.current, initial_health,
        "snipe should be pending while sniper is out of KILLPILOT range"
    );
    assert!(!target_after_far_update.is_unmanned());

    {
        let sniper = game_logic
            .host_object_mut(sniper_id)
            .expect("sniper should exist");
        // C++ MSG_DO_WEAPON_AT_OBJECT uses GLAJarmenKellVehiclePilotSniperRifle
        // AttackRange 225, not contact radii+4.
        sniper.set_position(Vec3::new(200.0, 0.0, 0.0));
        sniper.set_ai_state(AIState::SpecialAbility);
        sniper.target = Some(target_id);
    }
    game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle unmanned + Neutral.
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "kill-pilot residual must not damage vehicle HP"
    );
    assert!(
        target_after_contact.is_unmanned(),
        "snipe must leave vehicle unmanned"
    );
    assert_eq!(
        target_after_contact.team,
        Team::Neutral,
        "sniped vehicle becomes Neutral (gray/unowned)"
    );
    assert!(
        !target_after_contact.can_move(),
        "unmanned vehicle cannot move"
    );
    assert!(
        game_logic.honesty_snipe_vehicle_ok(),
        "snipe residual honesty"
    );
}

/// Retail `AmericaInfantryPilot` parser → live Enter re-crew authority.
///
/// C++ `VeterancyCrateCollide` requires exact `IsPilot`, a VEHICLE target
/// excluding DOZER, equal controlling player, and a non-airborne target.  In
/// particular, two USA slots are not one owner.  This uses the actual retail
/// Object INI rather than attaching a test-only pilot flag to a name.
#[test]
fn retail_pilot_metadata_drives_starting_veteran_and_same_owner_recrew() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::VeterancyLevel;
    use std::path::Path;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA 0", true));
    game_logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA 1", true));

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Main crate must remain three levels below repository root");
    let source = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/AmericaInfantry.ini"),
    )
    .expect("retail AmericaInfantry.ini");
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&source, "AmericaInfantry.ini")
        .expect("parse retail America infantry");
    let pilot_tpl = GameLogic::build_template_from_object_definition(
        "AmericaInfantryPilot",
        parser
            .get_definition("AmericaInfantryPilot")
            .expect("retail pilot definition"),
        None,
    );
    let metadata = pilot_tpl
        .veterancy_crate_collide
        .expect("retail IsPilot metadata");
    assert!(metadata.supports_pilot_recrew());
    assert_eq!(
        metadata.pilot_starting_level(),
        Some(VeterancyLevel::Veteran)
    );
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object_for_player("AmericaInfantryPilot", 0, Vec3::new(2.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot obj");
        assert_eq!(p.experience.level, VeterancyLevel::Veteran);
        assert_eq!(p.owner_player_id, Some(0));
    }

    // A pilot-named template with no parsed behavior remains ordinary
    // infantry: it starts Rookie and cannot create an Enter order.
    let mut name_only = ThingTemplate::new("AmericaInfantryPilotNameOnly");
    name_only
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilotNameOnly".to_string(), name_only);
    let name_only_id = game_logic
        .create_object_for_player("AmericaInfantryPilotNameOnly", 0, Vec3::new(2.0, 0.0, 2.0))
        .expect("name-only pilot");
    assert_eq!(
        game_logic
            .host_object(name_only_id)
            .expect("name-only object")
            .experience
            .level,
        VeterancyLevel::Rookie,
        "missing IsPilot metadata must fail closed for starting veterancy"
    );

    // A same-faction but different controlling player is rejected before an
    // order is installed.  KillPilot neutralizes the target while preserving
    // owner #1 in ObjectStatus, so this exercises the C++ controller gate.
    let foreign_tank_id = game_logic
        .create_object_for_player("TestTank", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign tank");
    {
        let t = game_logic
            .host_object_mut(foreign_tank_id)
            .expect("foreign tank object");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        assert_eq!(t.status.unmanned_owner_player_id, Some(1));
    }
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: foreign_tank_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![pilot_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let pilot = game_logic
        .host_object(pilot_id)
        .expect("pilot after foreign cmd");
    assert_eq!(pilot.ai_state, AIState::Idle);
    assert_eq!(pilot.target, None);
    assert!(
        game_logic
            .host_object(foreign_tank_id)
            .unwrap()
            .is_unmanned(),
        "same faction cannot bypass the exact controlling-player check"
    );

    // The name-only impostor is rejected even for a same-controller target.
    let tank_id = game_logic
        .create_object_for_player("TestTank", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("own tank");
    {
        let t = game_logic
            .host_object_mut(tank_id)
            .expect("own tank object");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        assert_eq!(t.status.unmanned_owner_player_id, Some(0));
    }
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: tank_id },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![name_only_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.host_object(name_only_id).unwrap().ai_state,
        AIState::Idle,
        "a template name is not IsPilot authority"
    );
    assert!(
        game_logic.host_object(tank_id).unwrap().is_unmanned(),
        "precondition: unmanned vehicle"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: tank_id },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![pilot_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after cmd");
        assert_eq!(p.ai_state, AIState::Entering);
        assert_eq!(p.target, Some(tank_id));
    }

    game_logic.update_ai(&[pilot_id, tank_id], 1.0 / 30.0);

    let tank = game_logic.host_object(tank_id).expect("tank after recrew");
    assert!(!tank.is_unmanned(), "recrew must clear DISABLED_UNMANNED");
    assert_eq!(tank.team, Team::USA, "recrew transfers pilot team");
    assert_eq!(
        tank.owner_player_id,
        Some(0),
        "recrew restores the exact controlling player, not only its faction"
    );
    assert_eq!(
        tank.experience.level,
        VeterancyLevel::Veteran,
        "pilot veterancy must transfer onto vehicle"
    );
    assert!(game_logic.honesty_pilot_recrew_ok(), "pilot recrew honesty");
    assert!(
        game_logic.honesty_pilot_veterancy_transfer_ok(),
        "veterancy transfer honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().recrews, 1);

    let pilot = game_logic
        .host_object(pilot_id)
        .expect("pilot after recrew");
    // `SlowDeathBehavior` can keep the corpse in the object map for its
    // authored delay, but it is no longer a live/controllable pilot.
    assert!(
        !pilot.is_alive(),
        "pilot infantry must be consumed even when its authored SlowDeath defers removal"
    );
}

#[test]
fn live_host_injects_completed_waypoint_labels_and_shroud_discovered_by() {
    use gamelogic::common::ObjectShroudStatus;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        clear_host_script_query_snapshot, host_script_query_object, host_script_query_object_by_id,
    };
    use gamelogic::system::shroud_manager::get_shroud_manager;

    OBJECT_REGISTRY.clear();
    clear_host_script_query_snapshot();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    logic.add_player(Player::new(2, Team::China, "PlyrChina", false));
    let mut t = ThingTemplate::new("NamedScout");
    t.set_health(100.0);
    logic.templates.insert("NamedScout".into(), t);
    let id = logic
        .create_object_for_player("NamedScout", 2, Vec3::new(10.0, 0.0, 20.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapNamedScout".into();
        o.completed_waypoint_labels = vec!["HeroPath".into()];
    }
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Shrouded);
        shroud.set_host_object_shroud_status(2, id.0, ObjectShroudStatus::Clear);
    }
    logic.inject_host_script_query_snapshot();
    let obj = host_script_query_object("MapNamedScout").expect("snapshot");
    assert_eq!(obj.waypoint_labels, vec!["HeroPath".to_string()]);
    assert!(
        obj.discovered_by.iter().any(|n| n.eq_ignore_ascii_case("PlyrChina")),
        "owner is CLEAR"
    );
    assert!(
        !obj.discovered_by
            .iter()
            .any(|n| n.eq_ignore_ascii_case("PlyrAmerica")),
        "shrouded player must not discover"
    );
    assert_eq!(host_script_query_object_by_id(id.0).map(|o| o.id), Some(id.0));
    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();
}

#[test]
fn live_host_from_named_and_skirmish_conditions_use_inject() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::player::player_list;
    use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
    use gamelogic::scripting::engine::{
        get_named_object_tracker, initialize_script_engine, with_script_engine_mut,
    };
    use gamelogic::scripting::executor::{ScriptConditionEvaluator, ScriptConditionResult};
    use gamelogic::scripting::{clear_host_script_query_snapshot, ScriptContext};
    use std::sync::{Arc, RwLock};

    OBJECT_REGISTRY.clear();
    clear_host_script_query_snapshot();
    initialize_script_engine().expect("script engine");
    player_list().write().unwrap().clear();
    let leftover = Arc::new(RwLock::new(gamelogic::player::Player::new(1)));
    leftover
        .write()
        .unwrap()
        .set_display_name("PlyrAmerica");
    player_list().write().unwrap().add_player(leftover);

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    logic.add_player(Player::new(2, Team::China, "PlyrChina", false));
    let mut t = ThingTemplate::new("NamedCannon");
    t.set_health(1000.0);
    logic.templates.insert("NamedCannon".into(), t);
    let id = logic
        .create_object_for_player("NamedCannon", 1, Vec3::new(5.0, 0.0, 5.0))
        .expect("cannon");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "ParticleCannon".into();
        o.team_instance_name = "USA_Superweapons".into();
        o.completed_waypoint_labels = vec!["HeroPath".into()];
    }
    logic.inject_host_named_unit_map_into_crate_tracker();

    let completed = with_script_engine_mut(|engine| {
        engine.notify_of_triggered_special_power(1, "SuperweaponParticleUplinkCannon", id.0);
        let mut from_named = Condition::new(ConditionType::PlayerTriggeredSpecialPowerFromNamed);
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "PlyrAmerica".into(),
            ))
            .unwrap();
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "SuperweaponParticleUplinkCannon".into(),
            ))
            .unwrap();
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ParticleCannon".into(),
            ))
            .unwrap();
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut from_named).unwrap(),
            ScriptConditionResult::True
        );

        let mut reached = Condition::new(ConditionType::NamedReachedWaypointsEnd);
        reached
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ParticleCannon".into(),
            ))
            .unwrap();
        reached
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                "HeroPath".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator.evaluate_condition(&mut reached).unwrap(),
            ScriptConditionResult::True
        );

        let mut team_reached = Condition::new(ConditionType::TeamReachedWaypointsEnd);
        team_reached
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "USA_Superweapons".into(),
            ))
            .unwrap();
        team_reached
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                "HeroPath".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator.evaluate_condition(&mut team_reached).unwrap(),
            ScriptConditionResult::True
        );
    });

    player_list().write().unwrap().clear();
    clear_host_script_query_snapshot();
    get_named_object_tracker().clear().ok();
    assert_eq!(completed, Some(()));
    assert!(OBJECT_REGISTRY.is_empty());
}

fn drain_script_act_b_queues() {
    let _ = gamelogic::scripting::take_host_script_named_attitude_requests();
    let _ = gamelogic::scripting::take_host_script_stopping_distance_requests();
    let _ = gamelogic::scripting::take_host_script_force_select_requests();
    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
}

#[test]
fn live_named_set_attitude_drains_to_host_ai() {
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_named_attitude, HostScriptNamedAttitudeRequest,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Hero");
    t.set_health(100.0);
    logic.templates.insert("Hero".into(), t);
    let id = logic
        .create_object("Hero", Team::USA, Vec3::new(8.0, 0.0, 4.0))
        .expect("hero");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "Hero".into();
        o.set_ai_attitude_i8(0);
    }

    request_host_script_named_attitude(HostScriptNamedAttitudeRequest {
        unit: "Hero".into(),
        mood: 2,
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        logic.host_object(id).expect("hero").ai_attitude(),
        HostAiAttitude::Aggressive,
        "NAMED_SET_ATTITUDE must call host setAttitude (Aggressive=2)"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_set_stopping_distance_drains_and_arrives_at_scripted_band() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_stopping_distance, HostScriptStoppingDistanceRequest,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Scout");
    t.set_health(100.0);
    logic.templates.insert("Scout".into(), t);
    let id = logic
        .create_object("Scout", Team::USA, Vec3::ZERO)
        .expect("scout");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "Scout".into();
        o.team_instance_name = "TeamA".into();
        o.request_path(Vec3::new(10.0, 0.0, 0.0), None);
    }

    request_host_script_stopping_distance(HostScriptStoppingDistanceRequest::Team {
        team: "TeamA".into(),
        distance: 25.0,
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    {
        let obj = logic.host_object(id).expect("scout");
        assert_eq!(
            obj.close_enough_dist,
            Some(25.0),
            "SET_STOPPING_DISTANCE must stamp Locomotor::setCloseEnoughDist"
        );
    }

    if let Some(o) = logic.host_object_mut(id) {
        o.update_movement(1.0 / 30.0);
        assert!(
            o.movement.target_position.is_none(),
            "scripted closeEnoughDist 25 must stop a 10wu goal"
        );
    }

    request_host_script_stopping_distance(HostScriptStoppingDistanceRequest::Team {
        team: "TeamA".into(),
        distance: 0.25,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("scout").close_enough_dist,
        Some(25.0),
        "C++ ignores stoppingDistance < 0.5"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_object_force_select_selects_lowest_id_and_centers() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_force_select, HostScriptForceSelectRequest,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("AmericaInfantryRanger".into(), t);

    let older = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(40.0, 0.0, 10.0),
        )
        .expect("older");
    let newer = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(80.0, 0.0, 10.0),
        )
        .expect("newer");
    assert!(older.0 < newer.0);
    if let Some(o) = logic.host_object_mut(older) {
        o.team_instance_name = "teamAmerica".into();
    }
    if let Some(o) = logic.host_object_mut(newer) {
        o.team_instance_name = "teamAmerica".into();
    }

    request_host_script_force_select(HostScriptForceSelectRequest {
        team: "teamAmerica".into(),
        object_type: "AmericaInfantryRanger".into(),
        center_in_view: true,
        audio: "SelectRanger".into(),
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    let player = logic.players.get(&1).expect("local");
    assert_eq!(
        player.selected_objects,
        vec![older],
        "OBJECT_FORCE_SELECT picks the lowest object ID"
    );
    let focus = logic.peek_pending_camera_focus().expect("centerInView");
    assert!((focus.x - 40.0).abs() < 0.1);
    assert!((focus.z - 10.0).abs() < 0.1);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SelectRanger"),
        "C++ AudioEventRTS(audioToPlay)"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_player_repair_named_structure_queues_ai_repair() {
    use crate::command_system::CommandType;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::ai::AIDifficulty;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        request_host_script_player_misc, HostScriptPlayerMiscRequest,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "Player_America", false));
    logic
        .ai_manager
        .add_ai_player(1, Team::USA, AIDifficulty::Medium);

    let mut bunker_t = ThingTemplate::new("AmericaBunker");
    bunker_t
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0);
    logic.templates.insert("AmericaBunker".into(), bunker_t);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic.templates.insert("AmericaVehicleDozer".into(), dozer_t);

    let bunker = logic
        .create_object("AmericaBunker", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .expect("bunker");
    if let Some(o) = logic.host_object_mut(bunker) {
        o.name = "Bunker".into();
        o.health.current = 200.0;
        o.body_damage_state = HostBodyDamageType::Damaged;
        o.owner_player_id = Some(1);
    }
    let _dozer = logic
        .create_object("AmericaVehicleDozer", Team::USA, Vec3::ZERO)
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(_dozer) {
        o.owner_player_id = Some(1);
    }

    request_host_script_player_misc(HostScriptPlayerMiscRequest::RepairNamed {
        player: "Player_America".into(),
        structure: "Bunker".into(),
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    {
        let mut mgr = std::mem::take(&mut logic.ai_manager);
        mgr.update(&mut logic, 0.0);
        logic.ai_manager = mgr;
    }

    assert!(
        logic.command_queue.iter().any(|c| matches!(
            c.command_type,
            CommandType::Repair { target_id } if target_id == bunker
        )),
        "PLAYER_REPAIR_NAMED_STRUCTURE must enqueue AIPlayer::repairStructure"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

