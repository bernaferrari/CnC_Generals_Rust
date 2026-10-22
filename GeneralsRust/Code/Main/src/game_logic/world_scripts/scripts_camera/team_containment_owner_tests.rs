//! CPP ScriptConditions1994 and ScriptEngine7965: canonical team containment.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use crate::command_system::{CommandType, GameCommand, ModifierKeys};
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};

/// All objects and both containment definitions enter through one receiving
/// catalog before the first live object is born.
fn receiving_world() -> GameLogic {
    let source = r#"
Object OwnedNamedInfantry
 KindOf = INFANTRY SELECTABLE CAN_ATTACK
 TransportSlotCount = 1
 Locomotor = SET_NORMAL BasicHumanLocomotor
 Locomotor = SET_PANIC RedguardLocomotor
 Body = ActiveBody Health
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface OwnedAI
 End
End
Object OwnedTeamTransport
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Health
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = TransportContain Cargo
  Slots = 2
  AllowInsideKindOf = INFANTRY
 End
End
Object OwnedScriptCave
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody Health
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = CaveContain Cargo
  ContainMax = 2
  CaveIndex = 0
 End
End
"#;
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "OwnedTeamContain.ini".into(),
                source.into(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "NamedOwner", true));
    world.add_player(Player::new(2, Team::China, "NamedVictim", false));
    world
}
fn spawn_named_pair(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let unit = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let target = world
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().name = "NamedUnit".into();
    world.host_object_mut(target).unwrap().name = "NamedTarget".into();
    let object = world.host_object_mut(unit).unwrap();
    object.set_formation(19, glam::Vec2::new(4.0, 6.0));
    object.max_shots_to_fire = 3;
    assert!(world.apply_unit_locomotor_set(unit, "panic"));
    assert_eq!(
        world
            .host_object(unit)
            .unwrap()
            .cur_locomotor_name
            .as_deref(),
        Some("RedguardLocomotor")
    );
    assert_eq!(
        world
            .host_object(unit)
            .unwrap()
            .jet_ai
            .cur_locomotor_set
            .as_deref(),
        Some("SET_PANIC")
    );
    (unit, target)
}
fn roster(owner: &GameLogic, member: ObjectId) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("ContainedTeam".into(), "NamedOwner".into(), false, None)
        .unwrap();
    let team = factory.create_inactive_team("ContainedTeam").unwrap();
    let mut team = team.write().unwrap();
    team.set_active();
    team.add_member(member.0);
    team.get_id()
}
#[test]
fn actual_transport_team_wait_observes_only_driving_roster_and_physical_containment() {
    let mut owner = receiving_world();
    let mut peer = receiving_world();
    let (unit, _target) = spawn_named_pair(&mut owner);
    let (peer_unit, _peer_target) = spawn_named_pair(&mut peer);
    assert_eq!(unit, peer_unit);
    let team = roster(&owner, unit);
    assert_eq!(team, roster(&peer, peer_unit));
    let transport = owner
        .create_object_for_player("OwnedTeamTransport", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    owner.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: transport,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![unit],
        modifier_keys: ModifierKeys::default(),
    });
    owner.process_commands();
    owner.update_ai(&[unit, transport], 1.0 / 30.0);
    assert_eq!(
        owner.host_object(unit).unwrap().contained_by,
        Some(transport)
    );
    let engine = ScriptEngine::new().unwrap();
    for all in [true, false] {
        assert_eq!(
            HostScriptExecutionDriver::with_interpreter(&mut owner, &engine).team_contained(
                "ContainedTeam",
                None,
                None,
                all
            ),
            ScriptOwnerQuery::Present(true)
        );
        assert_eq!(
            HostScriptExecutionDriver::with_interpreter(&mut peer, &engine).team_contained(
                "ContainedTeam",
                None,
                None,
                all
            ),
            ScriptOwnerQuery::Present(false)
        );
    }
    owner.host_object_mut(unit).unwrap().set_contained_by(None);
    owner.host_object_mut(unit).unwrap().ai_state = AIState::Exiting;
    assert_eq!(
        HostScriptExecutionDriver::with_interpreter(&mut owner, &engine).team_contained(
            "ContainedTeam",
            None,
            None,
            false
        ),
        ScriptOwnerQuery::Present(false)
    );
    assert_eq!(
        HostScriptExecutionDriver::with_interpreter(&mut owner, &engine)
            .team_contained("Missing", None, None, true),
        ScriptOwnerQuery::Missing
    );
}
#[test]
fn authored_cave_index_driver_updates_exact_owner_network_and_rejects_occupied_switch() {
    let mut owner = receiving_world();
    let mut peer = receiving_world();
    let (unit, _target) = spawn_named_pair(&mut owner);
    let _peer_pair = spawn_named_pair(&mut peer);
    let cave = owner
        .create_object_for_player("OwnedScriptCave", 1, Vec3::ZERO)
        .unwrap();
    let peer_cave = peer
        .create_object_for_player("OwnedScriptCave", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(cave, peer_cave);
    owner.host_object_mut(cave).unwrap().name = "CaveB".into();
    peer.host_object_mut(peer_cave).unwrap().name = "CaveB".into();
    let engine = ScriptEngine::new().unwrap();
    assert_eq!(
        HostScriptExecutionDriver::with_interpreter(&mut owner, &engine)
            .set_named_cave_index("CaveB", 3),
        Some(())
    );
    assert_eq!(owner.cave_system.index_of(cave), Some(3));
    assert_eq!(peer.cave_system.index_of(cave), Some(0));
    assert!(owner.cave_system.record_enter(3, unit, cave, Team::USA).0);
    HostScriptExecutionDriver::with_interpreter(&mut owner, &engine)
        .set_named_cave_index("CaveB", 4);
    assert_eq!(owner.cave_system.index_of(cave), Some(3));
}

#[test]
fn train_held_action_before_first_tick_is_owned_and_failed_restore_preserves_peer() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    use gamelogic::scripting::executor::ScriptContext;
    let (mut owner, _, _) = world();
    let (mut peer, _, _) = world();
    for world in [&mut owner, &mut peer] {
        let mut definition = ThingTemplate::new("CivilianTrainEngine");
        definition.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert(definition.name.clone(), definition);
    }
    let train = owner
        .create_object_for_player("CivilianTrainEngine", 1, Vec3::ZERO)
        .unwrap();
    let peer_train = peer
        .create_object_for_player("CivilianTrainEngine", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(train, peer_train);
    owner.host_object_mut(train).unwrap().name = "OwnedTrain".into();
    peer.host_object_mut(train).unwrap().name = "OwnedTrain".into();
    let engine = ScriptEngine::new().unwrap();
    let context = ScriptContext::at_frame(0);
    let mut action = ScriptAction::new(ScriptActionType::SetTrainHeld);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "OwnedTrain".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        None,
        context,
        &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
    );
    assert!(owner.railroad_car(train).unwrap().held);
    assert!(peer.railroad_car(train).is_none());
    peer.set_railroad_held(train, false);
    assert!(!peer.railroad_car(train).unwrap().held);
    let tail = capture_lifecycle_tail(&owner).expect("valid lifecycle capture");
    owner.set_railroad_held(train, false);
    apply_lifecycle_tail_to_host(&tail, &mut owner).unwrap();
    assert!(owner.railroad_car(train).unwrap().held);
    assert!(!peer.railroad_car(train).unwrap().held);
    let mut broken = tail.clone();
    let train_envelope = broken
        .envelopes
        .iter_mut()
        .find(|entry| entry.entity_id == train.0)
        .unwrap();
    let railroad = train_envelope
        .module_states
        .iter_mut()
        .find(|entry| entry.tag == "RailroadBehavior")
        .unwrap();
    railroad.payload = vec![0xFF];
    assert!(apply_lifecycle_tail_to_host(&broken, &mut owner).is_err());
    assert!(owner.railroad_car(train).unwrap().held);
    assert!(!peer.railroad_car(train).unwrap().held);
    owner.reset();
    assert!(owner.railroad_car(train).is_none());
    assert!(!peer.railroad_car(train).unwrap().held);
}

impl GameLogic {
    /// Exercise the actual borrowed action ingress from native train fixtures.
    pub(crate) fn execute_train_held_script_for_test(&mut self, name: &str, held: bool) {
        let engine = ScriptEngine::new().unwrap();
        let context = gamelogic::scripting::executor::ScriptContext::at_frame(self.frame);
        let mut action = ScriptAction::new(ScriptActionType::SetTrainHeld);
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, i32::from(held)))
            .unwrap();
        engine.friend_execute_action_with_driver(
            &action,
            None,
            context,
            &mut HostScriptExecutionDriver::with_interpreter(self, &engine),
        );
    }
}
