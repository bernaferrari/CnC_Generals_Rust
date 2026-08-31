//! Behavior suite extracted from `network_and_scripts`.
use super::*;

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
    assert!(
        !logic
            .host_area_unit_ids(Vec3::new(0.0, 0.0, 0.0), Vec3::new(15.0, 0.0, 25.0))
            .is_empty()
    );

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
        use gamelogic::scripting::{HostScriptQuerySnapshot, set_host_script_query_snapshot};
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
    use gamelogic::scripting::clear_host_script_query_snapshot;
    use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
    use gamelogic::scripting::engine::get_named_object_tracker;
    use gamelogic::scripting::executor::{
        ScriptConditionEvaluator, ScriptConditionResult, ScriptContext,
    };
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
fn unit_health_injects_leftover_initial_health_not_current_max() {
    // hq-um3v2: C++ evaluateUnitHealth divides by BodyModule::getInitialHealth,
    // not current max. Live inject used to send max, so INI InitialHealth 80 /
    // MaxHealth 100 at authored start fired at ~80% instead of 100%.
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
    use gamelogic::scripting::engine::get_named_object_tracker;
    use gamelogic::scripting::executor::{
        ScriptConditionEvaluator, ScriptConditionResult, ScriptContext,
    };
    use gamelogic::scripting::{clear_host_script_query_snapshot, host_script_query_object};
    use std::sync::{Arc, RwLock};

    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();
    get_named_object_tracker().clear().unwrap();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("HurtHero");
    t.set_health(100.0);
    logic.templates.insert("HurtHero".into(), t);
    let id = logic
        .create_object("HurtHero", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapHurtHero".into();
        o.initial_health = 80.0;
        o.health.current = 80.0;
        o.health.maximum = 100.0;
        o.max_health = 100.0;
    }
    logic.inject_host_named_unit_map_into_crate_tracker();

    let host = host_script_query_object("MapHurtHero").expect("injected");
    assert!(
        (host.initial_health - 80.0).abs() < 1e-4,
        "inject must send leftover InitialHealth, got {}",
        host.initial_health
    );
    assert!((host.health - 80.0).abs() < 1e-4);

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut ge = Condition::new(ConditionType::UnitHealth);
    ge.add_parameter(Parameter::with_string(
        ParameterType::Unit,
        "MapHurtHero".into(),
    ))
    .unwrap();
    ge.add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    ge.add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut ge).unwrap(),
        ScriptConditionResult::True,
        "authored start at InitialHealth is 100% vs leftover InitialHealth"
    );

    // Current-max denominator would report 80 and fail GREATER_EQUAL 100.
    if let Some(o) = logic.host_object_mut(id) {
        assert_eq!(o.unit_health_script_percent(), 100);
    }

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
fn host_player_census_excludes_kindof_inert_from_has_any_objects() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{clear_host_script_query_snapshot, host_query_player_census};

    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));

    let mut field = ThingTemplate::new("TestInertRadiationField");
    field.set_health(100.0);
    field.add_kind_of(KindOf::Inert);
    logic
        .templates
        .insert("TestInertRadiationField".into(), field);

    logic
        .create_object_for_player("TestInertRadiationField", 1, Vec3::new(10.0, 0.0, 20.0))
        .expect("radiation field");
    logic.inject_host_script_query_snapshot();
    let census = host_query_player_census("PlyrAmerica").expect("host census");
    assert!(
        !census.has_any_objects,
        "KINDOF_INERT keepalives must not stall PLAYER_ALL_DESTROYED"
    );

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_health(100.0);
    ranger.add_kind_of(KindOf::Infantry);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    logic
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(12.0, 0.0, 22.0))
        .expect("ranger");

    logic.inject_host_script_query_snapshot();
    let census = host_query_player_census("PlyrAmerica").expect("host census");
    assert!(
        census.has_any_objects,
        "a living non-inert unit still counts"
    );

    clear_host_script_query_snapshot();
}

#[test]
fn create_object_script_request_spawns_named_host_unit() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{HostScriptCreateRequest, request_host_script_create};

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
        HostScriptRadarEventRequest, HostScriptStealthEnabledRequest, HostScriptUnmannedRequest,
        request_host_script_radar_event, request_host_script_stealth_enabled,
        request_host_script_unmanned,
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
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);

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
        assert!(
            tank.status.disabled_unmanned,
            "SET_UNMANNED must stamp DISABLED_UNMANNED"
        );
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
        assert!(
            hero.script_unstealthed,
            "SET_STEALTH false stamps SCRIPT_UNSTEALTHED"
        );
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
    use gamelogic::scripting::{HostScriptRadarEventRequest, request_host_script_radar_event};

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
    use game_engine::common::well_known_keys::{
        key_team_name, key_team_on_create_script, key_team_on_unit_destroyed_script, key_team_owner,
    };
    use gamelogic::common::Dict;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::team::get_team_factory;

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
    let tick = include_str!("../../world_scripts/scripts_camera/script_runtime_camera.rs");
    let runtime = concat!(
        include_str!("../../mission_scripts/mod.rs"),
        include_str!("../../mission_scripts/script_requests.rs"),
        include_str!("../../mission_scripts/script_engine.rs"),
        include_str!("../../mission_scripts/script_hooks.rs"),
        include_str!("../../mission_scripts/script_actions.rs"),
        include_str!("../../mission_scripts/tests.rs"),
    );
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
    game_logic
        .guard_next_enemy_scan
        .insert(guard_id, game_logic.frame);
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
        (
            attacker.get_position(),
            attacker.team,
            attacker.can_attack(),
        )
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
        (
            attacker.get_position(),
            attacker.team,
            attacker.can_attack(),
        )
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
            assert_eq!(
                target_id, far_id,
                "Hunt must seek map-wide, not a 200 bubble"
            );
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

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after trigger");
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
    assert!(
        !lotus
            .special_power_cooldowns
            .contains_key(&crate::command_system::SpecialPowerType::BlackLotusCaptureBuilding)
    );
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
    use crate::game_logic::CapturePowerKind;
    use crate::game_logic::host_enum_table_residual::firing_a_model_bit;
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
