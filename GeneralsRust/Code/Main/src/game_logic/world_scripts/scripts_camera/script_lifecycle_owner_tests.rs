//! Original script lifecycle effects use the supplied world/interpreter.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn action(kind: ScriptActionType, name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            if matches!(
                kind,
                ScriptActionType::TeamDelete
                    | ScriptActionType::TeamDeleteLiving
                    | ScriptActionType::TeamKill
            ) {
                ParameterType::Team
            } else if kind == ScriptActionType::PlayerKill {
                ParameterType::Side
            } else {
                ParameterType::Unit
            },
            name.into(),
        ))
        .unwrap();
    action
}
fn execute(
    world: &mut GameLogic,
    engine: &gamelogic::scripting::engine::ScriptEngine,
    kind: ScriptActionType,
    name: &str,
) {
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(11);
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
    engine.friend_execute_action_with_driver(&action(kind, name), None, context, &mut driver);
}
fn roster(world: &mut GameLogic, name: &str, members: &[ObjectId]) {
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(name.into(), "NamedOwner".into(), false, None, Some(1))
            .unwrap();
        factory.create_team(name).unwrap()
    };
    for id in members {
        world.host_object_mut(*id).unwrap().team_instance_name = name.into();
        team.write().unwrap().add_member(id.0);
    }
    team.write().unwrap().set_active();
}

#[test]
fn named_delete_and_missing_kill_leave_same_id_peer_unchanged() {
    let (mut owner, id, target) = world();
    let (peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    execute(&mut owner, &engine, ScriptActionType::NamedKill, "Missing");
    assert!(owner.host_object(id).unwrap().is_alive());
    execute(
        &mut owner,
        &engine,
        ScriptActionType::NamedDelete,
        "NamedUnit",
    );
    assert!(owner.host_object(id).unwrap().status.destroyed);
    assert!(
        !owner.host_object(id).unwrap().status.on_die_started,
        "direct deletion does not run onDie"
    );
    assert!(owner.host_object(target).unwrap().is_alive());
    assert!(peer.host_object(peer_id).unwrap().is_alive());
}

#[test]
fn team_delete_living_uses_effectively_dead_state_instead_of_health() {
    let (mut owner, id, target) = world();
    roster(&mut owner, "OwnedLifecycle", &[id, target]);
    owner.host_object_mut(id).unwrap().health.current = 0.0;
    owner
        .host_object_mut(target)
        .unwrap()
        .status
        .effectively_dead = true;
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    execute(
        &mut owner,
        &engine,
        ScriptActionType::TeamDeleteLiving,
        "OwnedLifecycle",
    );
    assert!(
        owner.host_object(id).unwrap().status.destroyed,
        "zero HP alone is not effectively dead"
    );
    assert!(!owner.host_object(target).unwrap().status.destroyed);
}

#[test]
fn named_kill_topple_callback_reads_only_the_supplied_interpreter() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object AuthoredToppleTarget\n KindOf = STRUCTURE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StructureToppleUpdate ModuleTag_Fall\n End\nEnd\n", "owned-lifecycle.ini").unwrap();
    let template = GameLogic::build_template_from_object_definition(
        "AuthoredToppleTarget",
        parser.get_definition("AuthoredToppleTarget").unwrap(),
        None,
    );
    let mut owner = GameLogic::new();
    owner.templates.insert(template.name.clone(), template);
    let id = owner
        .create_object("AuthoredToppleTarget", Team::Neutral, Vec3::ZERO)
        .unwrap();
    owner.host_object_mut(id).unwrap().name = "ToppleTarget".into();
    let supplied = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let foreign = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    supplied.set_topple_direction(
        "ToppleTarget",
        Some(gamelogic::common::Coord3D::new(0.0, 1.0, 0.0)),
    );
    foreign.set_topple_direction(
        "ToppleTarget",
        Some(gamelogic::common::Coord3D::new(1.0, 0.0, 0.0)),
    );
    struct NestedKill<'a> {
        owner: &'a mut GameLogic,
        supplied: &'a gamelogic::scripting::engine::ScriptEngine,
    }
    impl gamelogic::scripting::engine::ScriptExecutionDriver for NestedKill<'_> {
        fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
            Ok(())
        }

        fn delete_or_kill(
            &mut self,
            _: &gamelogic::scripting::HostScriptKillDeleteDamageRequest,
            _: Option<gamelogic::common::ObjectID>,
            _: Option<gamelogic::team::TeamID>,
            _: Option<gamelogic::team::TeamID>,
        ) -> Option<gamelogic::GameLogicResult<()>> {
            // Public execution ingress installs the foreign interpreter while
            // the actual nested kill still receives the driving interpreter.
            execute(
                self.owner,
                self.supplied,
                ScriptActionType::NamedKill,
                "ToppleTarget",
            );
            Some(Ok(()))
        }
    }
    let mut nested = NestedKill {
        owner: &mut owner,
        supplied: &supplied,
    };
    foreign.friend_execute_action_with_driver(
        &action(ScriptActionType::NamedKill, "ToppleTarget"),
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(11),
        &mut nested,
    );
    let topple = owner
        .host_object(id)
        .unwrap()
        .structure_topple_data
        .as_ref()
        .unwrap();
    assert_eq!(topple.dir_x, 0.0);
    assert_eq!(topple.dir_y, 1.0);
}

#[test]
fn player_kill_targets_exact_same_faction_owner_and_missing_is_inert() {
    let (mut owner, id, _) = world();
    owner.add_player(Player::new(3, Team::USA, "SameFactionPeer", true));
    let other = owner
        .create_object_for_player("OwnedNamedInfantry", 3, Vec3::ZERO)
        .unwrap();
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    execute(&mut owner, &engine, ScriptActionType::PlayerKill, "Missing");
    assert!(owner.host_object(id).unwrap().is_alive());
    execute(
        &mut owner,
        &engine,
        ScriptActionType::PlayerKill,
        "NamedOwner",
    );
    assert!(!owner.host_object(id).unwrap().is_alive());
    assert!(owner.host_object(other).unwrap().is_alive());
    assert!(owner.players[&3].is_alive);
}
