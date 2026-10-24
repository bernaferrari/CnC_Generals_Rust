//! CPP AIStates5949 + ScriptActions6353: same-action interpreter policy.
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};

fn fixture(normal: bool) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.admit_session_difficulty(2);
    world.add_player(
        Player::new(1, Team::USA, "PolicyOwner", true).with_script_identity("PolicyOwner"),
    );
    world.add_player(
        Player::new(2, Team::GLA, "PolicyVictim", false).with_script_identity("PolicyVictim"),
    );
    // Authored controlling-player identities use explicit relationship facts.
    // Faction labels alone do not authorize an enemy mood scan.
    world
        .get_player_mut(1)
        .unwrap()
        .map_side
        .relations
        .insert(2, gamelogic::common::Relationship::Enemies);
    world
        .get_player_mut(2)
        .unwrap()
        .map_side
        .relations
        .insert(1, gamelogic::common::Relationship::Enemies);
    for name in ["PolicyUnit", "PolicyFar", "PolicyNear"] {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        template.set_authored_ai_update_interface(Some(true));
        world.templates.insert(name.into(), template);
    }
    let unit = world
        .create_object_for_player("PolicyUnit", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let far = world
        .create_object_for_player("PolicyFar", 2, Vec3::new(180.0, 0.0, 100.0))
        .unwrap();
    let near = world
        .create_object_for_player("PolicyNear", 2, Vec3::new(110.0, 0.0, 100.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().name = "PolicyUnit".into();
    world
        .owned_script_engine
        .as_ref()
        .unwrap()
        .set_choose_victim_always_uses_normal(normal);
    world
        .owned_script_engine
        .as_ref()
        .unwrap()
        .set_object_attack_priority_set(unit.0, "PolicyPriority");
    // Admit the definition through the production action and receiving content
    // lookup, so original ScriptEngine Xfer carries the real priority table.
    world
        .with_owned_script_engine(|engine, receiving| {
            assert!(engine.set_priority_default("PolicyPriority", 1));
            let mut action = ScriptAction::new(ScriptActionType::SetAttackPriorityThing);
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::AttackPrioritySet,
                    "PolicyPriority".into(),
                ))
                .unwrap();
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::ObjectType,
                    "PolicyFar".into(),
                ))
                .unwrap();
            action
                .add_parameter(Parameter::with_int(ParameterType::Int, 100))
                .unwrap();
            let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
            engine.friend_execute_action_with_driver(
                &action,
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut driver,
            );
            assert_eq!(
                engine
                    .get_attack_info("PolicyPriority")
                    .unwrap()
                    .priority_map
                    .get("PolicyFar"),
                Some(&100)
            );
        })
        .unwrap();
    world.initialize_receiving_named_cache_for_fixture();
    (world, unit, far, near)
}

fn assert_choices(world: &GameLogic, unit: ObjectId, expected: ObjectId) {
    assert_eq!(
        world.choose_attack_team_victim(unit, "teamPolicyVictim", true),
        Some(expected)
    );
    assert_eq!(world.find_closest_enemy(unit, 500.0, 0), Some(expected));
}

#[test]
fn source_frame_observers_import_priority_and_choose_from_receiving_policy() {
    let (mut first, unit, far, near) = fixture(true);
    let (mut peer, peer_unit, peer_far, _) = fixture(false);
    assert_eq!(unit, peer_unit);
    assert_eq!(far, peer_far);
    let mut seen = 0;
    first.tick_logic_frame_with_boundary(LOGIC_FRAME_TIMESTEP, None, Some(1), |receiving| {
        seen += 1;
        assert_eq!(
            receiving
                .host_object(unit)
                .unwrap()
                .attack_priority_set
                .as_deref(),
            Some("PolicyPriority")
        );
        assert_choices(receiving, unit, near);
        peer.tick_logic_frame_with_boundary(LOGIC_FRAME_TIMESTEP, None, Some(1), |other| {
            assert_eq!(
                other
                    .host_object(unit)
                    .unwrap()
                    .attack_priority_set
                    .as_deref(),
                Some("PolicyPriority")
            );
            assert_choices(other, unit, far);
        });
        assert_choices(receiving, unit, near);
    });
    assert_eq!(seen, 1);
    assert_choices(&peer, unit, far);
}

fn flag(value: bool) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::ChooseVictimAlwaysUsesNormal);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, i32::from(value)))
        .unwrap();
    action
}
fn attack() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::NamedAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "PolicyUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamPolicyVictim".into(),
        ))
        .unwrap();
    action
}
fn attack_button() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::NamedUseCommandbuttonAbility);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "PolicyUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "attackteam".into(),
        ))
        .unwrap();
    action
}
fn attack_team() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamPolicyOwner".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamPolicyVictim".into(),
        ))
        .unwrap();
    action
}
struct ObserveActions<'a> {
    actual: HostScriptExecutionDriver<'a>,
    unit: ObjectId,
    targets: Vec<Option<ObjectId>>,
}
impl ScriptExecutionDriver for ObserveActions<'_> {
    fn named_attack_team(
        &mut self,
        unit: &str,
        team: &str,
        object: Option<u32>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.actual
            .named_attack_team(unit, team, object, calling, condition)
    }
    fn team_attack_team(
        &mut self,
        source: &str,
        target: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.actual
            .team_attack_team(source, target, calling, condition)
    }
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        self.actual.after_action()?;
        self.targets
            .push(self.actual.world.host_object(self.unit).unwrap().target);
        Ok(())
    }
}

#[test]
fn held_interpreter_actions_change_victim_before_next_action_and_restore_owner() {
    let (mut world, unit, far, near) = fixture(false);
    let (peer, _, _, _) = fixture(false);
    world.host_object_mut(unit).unwrap().weapon = Some(Weapon {
        damage: 10.0,
        range: 500.0,
        can_target_ground: true,
        ..Weapon::default()
    });
    world
        .with_owned_script_engine(|engine, receiving| {
            assert!(receiving.owned_script_engine.is_none());
            let mut actions = flag(true);
            let mut first_attack = attack_team();
            let mut disable = flag(false);
            let mut second_attack = attack();
            let mut enable_again = flag(true);
            enable_again.next_action = Some(Box::new(attack_button()));
            second_attack.next_action = Some(Box::new(enable_again));
            disable.next_action = Some(Box::new(second_attack));
            first_attack.next_action = Some(Box::new(disable));
            actions.next_action = Some(Box::new(first_attack));
            let mut observer = ObserveActions {
                actual: HostScriptExecutionDriver::with_interpreter(receiving, engine),
                unit,
                targets: Vec::new(),
            };
            engine.friend_execute_action_with_driver(
                &actions,
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut observer,
            );
            assert_eq!(observer.targets.len(), 6);
            assert_eq!(observer.targets[1], Some(near));
            assert_eq!(observer.targets[3], Some(far));
            assert_eq!(
                observer.targets[5],
                Some(near),
                "after_action command-button dispatch reads the borrowed interpreter"
            );
            assert!(engine.get_choose_victim_always_uses_normal());
        })
        .unwrap();
    assert!(world.owned_script_engine.is_some());
    assert!(!peer.receiving_choose_victim_uses_normal());
    assert_eq!(peer.host_object(unit).unwrap().target, None);
}

#[test]
fn saved_policy_continues_and_peer_reset_does_not_select_another_interpreter() {
    use crate::save_load::snapshot::SnapshotBuilder;
    let (mut world, unit, far, near) = fixture(true);
    let (mut peer, _, _, _) = fixture(false);
    world.tick_logic_frame_with_boundary(LOGIC_FRAME_TIMESTEP, None, Some(1), |_| {});
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&world).unwrap();
    assert!(
        saved
            .persist_v18
            .script_engine_tail
            .as_ref()
            .unwrap()
            .choose_victim_always_uses_normal
    );
    let (mut restored, _, _, _) = fixture(false);
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    assert!(
        decoded
            .persist_v18
            .script_engine_tail
            .as_ref()
            .unwrap()
            .choose_victim_always_uses_normal
    );
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(restored.receiving_choose_victim_uses_normal());
    assert_choices(&restored, unit, near);
    peer.reset();
    assert!(!peer.receiving_choose_victim_uses_normal());
    assert_choices(&restored, unit, near);
    restored
        .owned_script_engine
        .as_ref()
        .unwrap()
        .set_choose_victim_always_uses_normal(false);
    assert_choices(&restored, unit, far);
    assert_choices(&world, unit, near);
}

#[test]
fn ordinary_command_executor_uses_receiving_interpreter_policy() {
    let (mut world, unit, far, near) = fixture(true);
    let (mut peer, _, _, _) = fixture(false);
    for owner in [&mut world, &mut peer] {
        owner.host_object_mut(unit).unwrap().weapon = Some(Weapon {
            damage: 10.0,
            range: 500.0,
            can_target_ground: true,
            ..Weapon::default()
        });
    }
    assert_eq!(
        crate::command_executor::CommandExecutor::new(&mut world, 1).execute_attack_team(
            &[unit],
            0,
            -1
        ),
        crate::command_system::CommandResult::Success
    );
    assert_eq!(world.host_object(unit).unwrap().target, Some(near));
    assert_eq!(
        crate::command_executor::CommandExecutor::new(&mut peer, 1).execute_attack_team(
            &[unit],
            0,
            -1
        ),
        crate::command_system::CommandResult::Success
    );
    assert_eq!(peer.host_object(unit).unwrap().target, Some(far));
    assert_eq!(world.host_object(unit).unwrap().target, Some(near));
}

#[test]
fn actual_easy_team_selection_consumes_driving_stream_with_foreign_scope_present() {
    let (mut world, unit, far, near) = fixture(false);
    let (peer, _, _, _) = fixture(false);
    world.admit_session_difficulty(0);
    let owner = world.get_player_mut(1).unwrap();
    owner.is_local = false;
    owner.is_human = false;
    world.logic_random.seed_random(37);
    peer.logic_random.seed_random(93);
    let before = world.logic_random.seed_words();
    let peer_before = peer.logic_random.seed_words();
    let selected =
        game_engine::common::random_value::with_session_random(&peer.logic_random, || {
            world.choose_attack_team_victim(unit, "teamPolicyVictim", false)
        });
    assert!(matches!(selected, Some(id) if id == far || id == near));
    assert_ne!(
        world.logic_random.seed_words(),
        before,
        "actual Easy selection draws from its driving owner"
    );
    assert_eq!(
        peer.logic_random.seed_words(),
        peer_before,
        "foreign callback scope is not the selector owner"
    );
}
