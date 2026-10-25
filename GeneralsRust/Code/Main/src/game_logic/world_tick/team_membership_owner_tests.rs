use super::super::*;

fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    // Test definitions make each synchronous source Strong impulse observable.
    world
        .admit_camera_application_shake(
            crate::presentation_frame::camera::shake_rules::CameraShakeRules {
                shake_strong_intensity: 2.0,
                max_shake_intensity: 100.0,
                max_shake_range: 1000.0,
                ..Default::default()
            },
        )
        .unwrap();
    world.add_player(
        Player::new(1, Team::USA, "RosterOwner", true).with_script_identity("RosterOwner"),
    );
    let mut template = ThingTemplate::new("RosterUnit");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    world
}
fn members(world: &GameLogic) -> Vec<u32> {
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    teams
        .iter()
        .flat_map(|team| team.read().unwrap().get_members().to_vec())
        .collect()
}
#[test]
fn actual_factory_admits_default_members_in_cpp_prepend_order_and_isolates_peer() {
    let mut world = owner();
    let peer = owner();
    let first = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(members(&world), [second.0, first.0]);
    world.activate_leftover_team_for_host_object(first);
    assert_eq!(
        members(&world),
        [second.0, first.0],
        "re-admission does not reorder"
    );
    assert!(members(&peer).is_empty());
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    assert_eq!(teams.len(), 1);
    assert!(teams[0].read().unwrap().is_active());
}
#[test]
fn existing_authored_default_identity_survives_player_admission() {
    let mut world = GameLogic::new();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "teamRosterOwner".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    let before = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1)[0]
        .read()
        .unwrap()
        .get_id();
    world.add_player(
        Player::new(1, Team::USA, "RosterOwner", true).with_script_identity("RosterOwner"),
    );
    let teams = world
        .team_factory
        .lock()
        .unwrap()
        .owned_team_instances_for_controller(1);
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].read().unwrap().get_id(), before);
}

#[test]
fn explicit_named_admission_moves_member_out_of_default_before_observation() {
    let mut world = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "NamedRoster".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    world.host_object_mut(unit).unwrap().team_instance_name = "NamedRoster".into();
    world.activate_leftover_team_for_host_object(unit);
    let mut factory = world.team_factory.lock().unwrap();
    assert!(
        !factory
            .find_team("teamRosterOwner")
            .unwrap()
            .read()
            .unwrap()
            .has_member(unit.0)
    );
    assert_eq!(
        factory
            .find_team("NamedRoster")
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        &[unit.0]
    );
}

#[test]
fn missing_exact_identity_does_not_retarget_an_existing_same_name_team() {
    let mut world = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    let before = members(&world);
    world.host_object_mut(unit).unwrap().team_instance_id = Some(u32::MAX);
    world.activate_leftover_team_for_host_object(unit);
    assert_eq!(
        world.host_object(unit).unwrap().team_instance_id,
        Some(u32::MAX)
    );
    assert_eq!(members(&world), before);
}

fn camera_create_prototype(world: &mut GameLogic, name: &str, singleton: bool) {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "OnCreate");
    let (_, effects) = world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned_with_effects(
            name.into(),
            "RosterOwner".into(),
            singleton,
            Some(&dict),
            Some(1),
        )
        .unwrap();
    // Singleton init itself creates its initial inactive instance. This fixture
    // installs the interpreter afterwards, so consume that setup effect here.
    assert_eq!(effects.len(), usize::from(singleton));
}

fn camera_create_engine() -> gamelogic::scripting::engine::ScriptEngine {
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    let mut action = ScriptAction::new(ScriptActionType::ScreenShake);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "OnCreate".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn native_authored_merge_uses_borrowed_interpreter_then_retires_source_and_activates_destination() {
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
    let mut world = owner();
    let peer = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "SourceTeam".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    world.host_object_mut(unit).unwrap().team_instance_name = "SourceTeam".into();
    world.host_object_mut(unit).unwrap().team_instance_id = None;
    world.activate_leftover_team_for_host_object(unit);
    let source_id = world.host_object(unit).unwrap().team_instance_id.unwrap();
    camera_create_prototype(&mut world, "CreatedTeam", false);
    world.install_owned_script_engine(camera_create_engine());
    let mut merge = ScriptAction::new(ScriptActionType::TeamMergeIntoTeam);
    for name in ["SourceTeam", "CreatedTeam"] {
        merge
            .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
            .unwrap();
    }
    world
        .with_owned_script_engine(|engine, world| {
            assert!(
                world.owned_script_engine.is_none(),
                "exercise actual borrowed interpreter"
            );
            world.execute_script_team_actions(engine, &merge, None);
            assert_eq!(
                world.session_view.impulse.intensity, 2.0,
                "callback completes before authored action returns"
            );
        })
        .unwrap();
    assert!(world.owned_script_engine.is_some());
    let factory = world.team_factory.lock().unwrap();
    assert!(factory.find_team_by_id(source_id).is_none());
    let team = factory.find_team_instances("CreatedTeam").remove(0);
    let team = team.read().unwrap();
    assert!(team.is_active());
    assert_eq!(team.get_members(), [unit.0]);
    assert_eq!(
        world.host_object(unit).unwrap().team_instance_id,
        Some(team.get_id())
    );
    assert_eq!(peer.session_view.impulse.intensity, 0.0);
    assert!(peer.pending_screen_shakes.is_empty());
}

#[test]
fn native_team_implicit_find_runs_create_shake_in_receiving_interpreter_once() {
    let mut world = owner();
    let peer = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    camera_create_prototype(&mut world, "CreatedTeam", false);
    world.install_owned_script_engine(camera_create_engine());
    world.host_object_mut(unit).unwrap().team_instance_name = "CreatedTeam".into();
    world.host_object_mut(unit).unwrap().team_instance_id = None;
    world.activate_leftover_team_for_host_object(unit);
    assert_eq!(world.session_view.impulse.intensity, 2.0);
    assert!(
        world.pending_screen_shakes.is_empty(),
        "borrowed callback must not leave a delayed duplicate"
    );
    world.activate_leftover_team_for_host_object(unit);
    assert_eq!(world.session_view.impulse.intensity, 2.0);
    assert_eq!(peer.session_view.impulse.intensity, 0.0);
    assert!(peer.pending_screen_shakes.is_empty());
}

#[test]
fn native_nested_authored_create_keeps_current_interpreter_and_exact_team_identity() {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    let mut world = owner();
    let peer = owner();
    camera_create_prototype(&mut world, "OuterTeam", false);
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "InnerCreate");
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned_with_effects(
            "InnerTeam".into(),
            "RosterOwner".into(),
            false,
            Some(&dict),
            Some(1),
        )
        .unwrap();
    let mut create = ScriptAction::new(ScriptActionType::CreateObject);
    create
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "RosterUnit".into(),
        ))
        .unwrap();
    create
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "InnerTeam".into(),
        ))
        .unwrap();
    create
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::common::Coord3D::new(10.0, 20.0, 0.0),
        ))
        .unwrap();
    create
        .add_parameter(Parameter::with_real(ParameterType::Real, 0.0))
        .unwrap();
    let mut outer_script = Script::new();
    outer_script.script_name = "OnCreate".into();
    outer_script.is_subroutine = true;
    outer_script.action = Some(Box::new(create));
    let mut shake = ScriptAction::new(ScriptActionType::ScreenShake);
    shake
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    let mut inner_script = Script::new();
    inner_script.script_name = "InnerCreate".into();
    inner_script.is_subroutine = true;
    inner_script.action = Some(Box::new(shake));
    let mut list = ScriptList::new();
    list.append_script(Box::new(outer_script));
    list.append_script(Box::new(inner_script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(engine);
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world.host_object_mut(unit).unwrap().team_instance_name = "OuterTeam".into();
    world.host_object_mut(unit).unwrap().team_instance_id = None;
    world
        .with_owned_script_engine(|engine, world| {
            world.admit_team_for_host_object(unit, Some(engine), true);
            assert_eq!(world.session_view.impulse.intensity, 2.0);
            let factory = world.team_factory.lock().unwrap();
            let inner = factory.find_team_instances("InnerTeam").remove(0);
            let inner = inner.read().unwrap();
            assert_eq!(inner.get_members().len(), 1);
            let member = ObjectId(inner.get_members()[0]);
            assert_ne!(member, unit);
            assert_eq!(
                world.host_object(member).unwrap().team_instance_id,
                Some(inner.get_id())
            );
        })
        .unwrap();
    assert_eq!(peer.session_view.impulse.intensity, 0.0);
    assert!(peer.pending_screen_shakes.is_empty());
    assert!(world.owned_script_engine.is_some());
}

#[test]
fn native_death_notification_never_creates_or_retargets_a_same_name_team() {
    let mut world = owner();
    let unit = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    camera_create_prototype(&mut world, "MissingResident", false);
    world.host_object_mut(unit).unwrap().team_instance_name = "MissingResident".into();
    world.host_object_mut(unit).unwrap().team_instance_id = None;
    world.notify_leftover_team_of_host_object_death(unit);
    assert!(
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_instances("MissingResident")
            .is_empty(),
        "death lookup cannot create a non-singleton or run its create action"
    );
    let (first, second) = {
        let mut factory = world.team_factory.lock().unwrap();
        (
            factory
                .create_inactive_team_with_owned_effects("MissingResident")
                .unwrap()
                .0,
            factory
                .create_inactive_team_with_owned_effects("MissingResident")
                .unwrap()
                .0,
        )
    };
    first.write().unwrap().add_member(unit.0);
    second.write().unwrap().add_member(unit.0);
    let identity = first.read().unwrap().get_id();
    world.host_object_mut(unit).unwrap().team_instance_id = Some(identity);
    world.notify_leftover_team_of_host_object_death(unit);
    assert!(!first.read().unwrap().has_member(unit.0));
    assert!(
        second.read().unwrap().has_member(unit.0),
        "newer same-name sibling is not the resident team"
    );
    assert_eq!(world.session_view.impulse.intensity, 0.0);
    assert!(world.pending_screen_shakes.is_empty());
}

#[test]
fn native_authored_reinforcement_keeps_condition_team_and_runs_callback_before_final_activation() {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList, THIS_TEAM,
    };
    use gamelogic::system::map_loader::{MapData, MapWaypoint};
    let mut world = owner();
    let peer = owner();
    let source = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "ConditionTeam".into(),
            "RosterOwner".into(),
            true,
            None,
            Some(1),
        )
        .unwrap();
    world.host_object_mut(source).unwrap().team_instance_name = "ConditionTeam".into();
    world.host_object_mut(source).unwrap().team_instance_id = None;
    world.activate_leftover_team_for_host_object(source);
    camera_create_prototype(&mut world, "ReinforcementTeam", true);
    let sentinel = world
        .create_object_for_player("RosterUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    world.host_object_mut(sentinel).unwrap().team_instance_name = "ReinforcementTeam".into();
    world.host_object_mut(sentinel).unwrap().team_instance_id = None;
    world.admit_team_for_host_object(sentinel, None, false);
    assert!(
        !world
            .team_factory
            .lock()
            .unwrap()
            .find_team_instances("ReinforcementTeam")[0]
            .read()
            .unwrap()
            .is_active()
    );
    let mut prototype = (*world
        .team_factory
        .lock()
        .unwrap()
        .find_team_prototype("ReinforcementTeam")
        .unwrap())
    .clone();
    prototype.set_units_info(
        0,
        gamelogic::team::CreateUnitsInfo {
            min_units: 2,
            max_units: 2,
            unit_thing_name: "RosterUnit",
        },
    );
    world
        .team_factory
        .lock()
        .unwrap()
        .replace_team_prototype(prototype);
    let mut map = MapData::new();
    map.width = 16;
    map.height = 16;
    map.heightmap = vec![0; 256];
    map.waypoints.push(MapWaypoint {
        id: 1,
        name: "Arrival".into(),
        location: gamelogic::common::Coord3D::new(20.0, 30.0, 0.0),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    let mut callback = Script::new();
    callback.script_name = "OnCreate".into();
    callback.is_subroutine = true;
    let mut guard = ScriptAction::new(ScriptActionType::TeamGuard);
    guard
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    let mut shake = ScriptAction::new(ScriptActionType::ScreenShake);
    shake
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    guard.next_action = Some(Box::new(shake));
    let mut named_inactive_guard = ScriptAction::new(ScriptActionType::TeamGuard);
    named_inactive_guard
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ReinforcementTeam".into(),
        ))
        .unwrap();
    named_inactive_guard.next_action = Some(Box::new(guard));
    callback.action = Some(Box::new(named_inactive_guard));
    let mut action = ScriptAction::new(ScriptActionType::CreateReinforcementTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ReinforcementTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "Arrival".into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut main = Script::new();
    main.script_name = "ActualReinforcement".into();
    main.condition_team_name = "ConditionTeam".into();
    main.is_one_shot = true;
    main.condition = Some(Box::new(condition));
    main.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(main));
    list.append_script(Box::new(callback));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
    assert_eq!(world.session_view.impulse.intensity, 2.0);
    assert_eq!(
        world.host_object(source).unwrap().ai_state(),
        AIState::GuardingArea,
        "null-calling-team create action retains the outer condition team"
    );
    let factory = world.team_factory.lock().unwrap();
    let instances = factory.find_team_instances("ReinforcementTeam");
    assert_eq!(instances.len(), 1);
    let team = instances[0].read().unwrap();
    assert!(team.is_active());
    assert_eq!(team.get_members().len(), 3);
    assert_eq!(
        world.host_object(sentinel).unwrap().ai_state(),
        AIState::Idle,
        "reused singleton remains inactive during create callback; named-team lookup must not guard its resident member"
    );
    for raw in team.get_members() {
        assert_eq!(
            world.host_object(ObjectId(*raw)).unwrap().team_instance_id,
            Some(team.get_id())
        );
    }
    assert_eq!(peer.session_view.impulse.intensity, 0.0);
    assert!(peer.pending_screen_shakes.is_empty());
    assert!(
        peer.team_factory
            .lock()
            .unwrap()
            .find_team_instances("ReinforcementTeam")
            .is_empty()
    );
    assert!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_condition_team_id()
            .is_none()
    );
}

#[test]
fn native_merge_missing_source_still_creates_inactive_destination_and_runs_receiving_callback() {
    let mut world = owner();
    let peer = owner();
    camera_create_prototype(&mut world, "MissingSourceDestination", false);
    world.install_owned_script_engine(camera_create_engine());
    world
        .with_owned_script_engine(|engine, world| {
            world
                .merge_owned_script_team(
                    "AbsentSource",
                    "MissingSourceDestination",
                    None,
                    None,
                    Some(engine),
                )
                .unwrap();
        })
        .unwrap();
    assert_eq!(world.session_view.impulse.intensity, 2.0);
    assert!(
        world.pending_screen_shakes.is_empty(),
        "borrowed callback must not leave a delayed duplicate"
    );
    let factory = world.team_factory.lock().unwrap();
    let instances = factory.find_team_instances("MissingSourceDestination");
    assert_eq!(instances.len(), 1);
    let destination = instances[0].read().unwrap();
    assert!(
        !destination.is_active(),
        "null-source return occurs before final activation"
    );
    assert!(destination.get_members().is_empty());
    assert!(factory.find_team_instances("AbsentSource").is_empty());
    assert_eq!(peer.session_view.impulse.intensity, 0.0);
    assert!(peer.pending_screen_shakes.is_empty());
}

#[test]
fn native_exact_team_creation_keeps_members_inactive_until_assembly_finishes() {
    let mut world = owner();
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "AssemblyTeam".into(),
                "RosterOwner".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        let (team, effects) = factory
            .create_inactive_team_with_owned_effects("AssemblyTeam")
            .unwrap();
        assert!(effects.is_empty());
        team
    };
    let identity = team.read().unwrap().get_id();
    for x in [0.0, 10.0] {
        let id = world
            .create_object_on_exact_script_team(
                "RosterUnit",
                1,
                "AssemblyTeam",
                identity,
                glam::Vec3::new(x, 0.0, 0.0),
                false,
                None,
            )
            .unwrap();
        assert_eq!(
            world.host_object(id).unwrap().team_instance_id,
            Some(identity)
        );
        assert!(team.read().unwrap().has_member(id.0));
        assert!(
            !team.read().unwrap().is_active(),
            "ThingFactory::newObject must not activate an assembling reinforcement team"
        );
    }
    assert_eq!(team.read().unwrap().get_members().len(), 2);
}
