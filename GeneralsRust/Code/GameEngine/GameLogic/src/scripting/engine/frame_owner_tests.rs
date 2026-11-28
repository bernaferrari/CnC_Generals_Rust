use super::*;
use crate::scripting::executor::ScriptContext;
use crate::system::engine_stores::{WorldServices, with_world_services};
use game_engine::common::common_services::ApplicationChannels;
use game_engine::common::message_stream::GameMessageType;

fn isolated_world() -> Arc<WorldServices> {
    Arc::new(WorldServices::in_application(Arc::new(
        ApplicationChannels::new(),
    )))
}

struct EndGameDriver {
    ran_teams: bool,
}

impl ScriptExecutionDriver for EndGameDriver {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn update_team_states(&mut self, _: &ScriptInterpreter) -> Option<GameLogicResult<()>> {
        self.ran_teams = true;
        Some(Ok(()))
    }
}

#[test]
fn countdowns_pause_at_mission_end_and_clear_routes_to_the_engine_application() {
    let owner = isolated_world();
    let peer = isolated_world();
    let engine = ScriptEngine::new_with_services(&owner).unwrap();
    let other_engine = ScriptEngine::new_with_services(&peer).unwrap();
    engine.set_timer("authored", 7).unwrap();
    other_engine.set_timer("authored", 11).unwrap();
    engine.set_fade_parameters(TFade::Multiply, 0.0, 1.0, 4, 0, 0);
    engine.start_quick_end_game_timer();
    let mut driver = EndGameDriver { ran_teams: false };

    with_world_services(&peer, || {
        engine
            .update_with_driver(ScriptContext::at_frame(1), &mut driver)
            .unwrap();
    });

    // ScriptEngine.cpp:5530–5548 advances the fade, then returns ahead of
    // the authored counters and side/team/sequential phases.
    assert_eq!(engine.get_fade_value(), 0.25);
    assert_eq!(engine.get_counter("authored").unwrap().value, 7);
    assert_eq!(other_engine.get_counter("authored").unwrap().value, 11);
    assert!(!driver.ran_teams);
    let has_clear = |world: &WorldServices| {
        world
            .common()
            .application
            .message_stream
            .read()
            .unwrap()
            .contains_message_of_type(&GameMessageType::ClearGameData)
    };
    assert!(has_clear(&owner));
    assert!(!has_clear(&peer));

    let saved = engine.snapshot_xfer_tail();
    let restored = ScriptEngine::new_with_services(&owner).unwrap();
    restored.restore_receiving_xfer_tail(&saved);
    let (counters, flags) = engine.snapshot_named_trackers();
    restored.restore_named_trackers(&counters, &flags).unwrap();
    owner
        .common()
        .application
        .message_stream
        .write()
        .unwrap()
        .clear_messages();
    with_world_services(&peer, || {
        restored
            .update_with_driver(ScriptContext::at_frame(2), &mut driver)
            .unwrap();
    });
    assert_eq!(restored.get_fade_value(), 0.5);
    assert_eq!(restored.get_counter("authored").unwrap().value, 7);
    assert!(!has_clear(&owner));
    assert!(!has_clear(&peer));
}

#[test]
fn authored_difficulty_uses_the_owning_player_and_name_namespace() {
    use crate::player::{GameDifficulty, Player};

    let owner = isolated_world();
    let peer = isolated_world();
    let owner_engine = ScriptEngine::new_with_services(&owner).unwrap();
    let peer_engine = ScriptEngine::new_with_services(&peer).unwrap();
    peer.common()
        .name_key_namespace
        .lock()
        .unwrap()
        .name_to_key("Unrelated name");
    let mut keys = Vec::new();
    for (world, engine, difficulty) in [
        (&owner, &owner_engine, GameDifficulty::Easy),
        (&peer, &peer_engine, GameDifficulty::Hard),
    ] {
        let key = world
            .common()
            .name_key_namespace
            .lock()
            .unwrap()
            .name_to_key("Authored side");
        keys.push(key);
        let mut player = Player::new_for_world(0, world);
        player.set_player_name_key(key);
        player.set_difficulty(difficulty);
        world
            .players()
            .write()
            .unwrap()
            .add_player(Arc::new(RwLock::new(player)));

        let mut condition = OrCondition::new();
        condition.first_and = Some(Box::new(Condition::new(ConditionType::ConditionTrue)));
        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "accepted".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        let mut script = Script::new();
        script.easy = difficulty == GameDifficulty::Easy;
        script.normal = false;
        script.hard = difficulty == GameDifficulty::Hard;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
    }
    assert_ne!(keys[0], keys[1]);

    for (engine, foreign) in [(&owner_engine, &peer), (&peer_engine, &owner)] {
        let mut driver = EndGameDriver { ran_teams: false };
        with_world_services(foreign, || {
            engine.update_with_driver(ScriptContext::at_frame(1), &mut driver)
        })
        .unwrap();
        assert!(engine.get_flag("accepted").unwrap().value);
    }
}

#[test]
fn sequential_team_observes_the_engine_world_inside_a_peer_scope() {
    use crate::scripting::{HostScriptQuery, HostScriptQueryObject, HostScriptQuerySnapshot};
    use crate::system::engine_stores::WorldServices;

    fn install_team(
        world: &Arc<WorldServices>,
        engine: &ScriptEngine,
        name: &str,
        idle: bool,
    ) -> TeamID {
        let id = with_world_services(world, || {
            let mut factory = world.team_factory().lock().unwrap();
            factory.init_team(name.into(), AsciiString::default(), false, None);
            let team = factory.create_team(name).unwrap();
            let id = team.read().unwrap().get_id();
            id
        });
        HostScriptQuery::of(world).set_host_script_query_snapshot(HostScriptQuerySnapshot {
            objects: vec![HostScriptQueryObject {
                id: 42,
                alive: true,
                idle,
                ..Default::default()
            }],
            team_instance_ids: [(name.to_owned(), vec![42])].into_iter().collect(),
            ..Default::default()
        });

        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "sequence_ran".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        let mut script = Script::new();
        script.script_name = name.into();
        script.action = Some(Box::new(action));
        let mut sequence = SequentialScript::new();
        sequence.team_to_exec_on = Some(id);
        sequence.script_to_execute_sequentially = Some(Box::new(script));
        engine.append_sequential_script(sequence);
        id
    }

    let owner = isolated_world();
    let peer = isolated_world();
    let engine = ScriptEngine::new_with_services(&owner).unwrap();
    let peer_engine = ScriptEngine::new_with_services(&peer).unwrap();
    let owner_team = install_team(&owner, &engine, "BusyTeam", false);
    let peer_team = install_team(&peer, &peer_engine, "IdleTeam", true);
    assert_eq!(owner_team, peer_team);

    let mut driver = EndGameDriver { ran_teams: false };
    let ran = |engine: &ScriptEngine| {
        engine
            .get_flag("sequence_ran")
            .is_some_and(|flag| flag.value)
    };
    with_world_services(&peer, || {
        engine
            .update_with_driver(ScriptContext::at_frame(1), &mut driver)
            .unwrap();
    });
    assert!(
        !ran(&engine),
        "the busy receiving team must not advance using the peer's team ID"
    );
    assert!(!ran(&peer_engine));
    assert_eq!(
        engine.snapshot_sequential_scripts()[0].current_instruction,
        -1
    );
    with_world_services(&owner, || {
        peer_engine
            .update_with_driver(ScriptContext::at_frame(1), &mut driver)
            .unwrap();
    });
    assert!(ran(&peer_engine));
    assert!(!ran(&engine));
}
