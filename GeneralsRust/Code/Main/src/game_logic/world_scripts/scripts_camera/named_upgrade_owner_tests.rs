//! CPP ScriptActions5315: synchronous giveUpgrade on the actual named receiver.
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};
use gamelogic::scripting::executor::ScriptContext;
const KEY: &str = "Upgrade_CostReduction";
const ACTOR: &str = "SameNamedUpgradeActor";
fn world(admit: bool) -> (GameLogic, ObjectId) {
    let mut owner = GameLogic::new();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner.add_player(Player::new(1, Team::USA, "FirstFactionPeer", true));
    owner.add_player(Player::new(2, Team::USA, "ActualController", false));
    let mut template = ThingTemplate::new("NamedUpgradeActor");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    owner.templates.insert(template.name.clone(), template);
    if admit {
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &owner, KEY, "OBJECT", 500, 30,
        );
    }
    let id = owner
        .create_object_for_player("NamedUpgradeActor", 2, Vec3::ZERO)
        .unwrap();
    owner.host_object_mut(id).unwrap().name = ACTOR.into();
    owner.initialize_receiving_named_cache_for_fixture();
    (owner, id)
}
fn action(unit: &str, upgrade: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, unit.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            upgrade.into(),
        ))
        .unwrap();
    action
}
struct NextActionProbe<'a> {
    inner: HostScriptExecutionDriver<'a>,
    id: ObjectId,
    observed: bool,
}
impl ScriptExecutionDriver for NextActionProbe<'_> {
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        self.inner.after_action()
    }
    fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
        self.inner.object_count_changed_frame()
    }
    fn named_upgrade(
        &mut self,
        name: &str,
        upgrade: &str,
        this: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.inner.named_upgrade(name, upgrade, this)
    }
    fn named_damage(
        &mut self,
        name: &str,
        amount: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        assert!(
            self.inner.interpreter.is_some(),
            "callback keeps the actual held interpreter"
        );
        assert!(self.inner.world.owned_script_engine.is_none());
        assert!(
            self.inner
                .world
                .host_object(self.id)
                .unwrap()
                .has_upgrade_tag(KEY),
            "giveUpgrade must finish before the next authored action"
        );
        let actual = self
            .inner
            .world
            .get_player(2)
            .unwrap()
            .production_cost_factor(&["VEHICLE"]);
        assert!((actual - 0.9).abs() < 0.0001);
        assert_eq!(
            self.inner
                .world
                .get_player(1)
                .unwrap()
                .production_cost_factor(&["VEHICLE"]),
            1.0,
            "same-faction first player is not the recipient"
        );
        self.observed = true;
        self.inner.named_damage(name, amount, this_object)
    }
}
#[test]
fn actual_action_chain_applies_before_next_callback_on_retained_controller_and_no_paid_research() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_action_chain_applies_before_next_callback_on_retained_controller_and_no_paid_research",
        || {
            let (mut owner, id) = world(true);
            let (peer, peer_id) = world(true);
            assert_eq!(id, peer_id);
            let cash = [
                owner.get_player(1).unwrap().resources.supplies,
                owner.get_player(2).unwrap().resources.supplies,
            ];
            let mut grant = action(ACTOR, KEY);
            let mut hit = ScriptAction::new(ScriptActionType::NamedDamage);
            hit.add_parameter(Parameter::with_string(ParameterType::Unit, ACTOR.into()))
                .unwrap();
            hit.add_parameter(Parameter::with_int(ParameterType::Int, 1))
                .unwrap();
            grant.set_next_action(Some(Box::new(hit)));
            owner
                .with_owned_script_engine(|engine, receiving| {
                    let mut probe = NextActionProbe {
                        inner: HostScriptExecutionDriver::with_interpreter(receiving, engine),
                        id,
                        observed: false,
                    };
                    engine
                        .friend_execute_action_with_driver_checked(
                            &grant,
                            None,
                            ScriptContext::at_frame(0),
                            &mut probe,
                        )
                        .unwrap();
                    assert!(probe.observed);
                })
                .unwrap();
            assert!(!peer.host_object(peer_id).unwrap().has_upgrade_tag(KEY));
            assert_eq!(
                peer.get_player(2)
                    .unwrap()
                    .production_cost_factor(&["VEHICLE"]),
                1.0
            );
            assert_eq!(
                [
                    owner.get_player(1).unwrap().resources.supplies,
                    owner.get_player(2).unwrap().resources.supplies
                ],
                cash
            );
            assert!(owner.get_player(2).unwrap().queued_upgrades.is_empty());
            assert!(
                !owner
                    .get_player(2)
                    .unwrap()
                    .completed_upgrades
                    .contains(KEY),
                "named give is not paid player research"
            );
        },
    );
}
#[test]
fn unknown_own_definition_and_missing_object_are_authoritative_while_this_uses_actual_id() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "unknown_own_definition_and_missing_object_are_authoritative_while_this_uses_actual_id",
        || {
            let (mut owner, id) = world(false);
            let (peer, peer_id) = world(true);
            let engine = ScriptEngine::new().unwrap();
            {
                let mut driver = HostScriptExecutionDriver::with_interpreter(&mut owner, &engine);
                assert!(driver.named_upgrade(ACTOR, KEY, None).unwrap().is_ok());
                assert!(
                    driver
                        .named_upgrade("AbsentNamedUpgradeActor", KEY, None)
                        .unwrap()
                        .is_ok()
                );
            }
            assert!(!owner.host_object(id).unwrap().has_upgrade_tag(KEY));
            assert!(!peer.host_object(peer_id).unwrap().has_upgrade_tag(KEY));
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &owner, KEY, "OBJECT", 500, 30,
            );
            {
                let mut driver = HostScriptExecutionDriver::with_interpreter(&mut owner, &engine);
                assert!(
                    driver
                        .named_upgrade(gamelogic::scripting::core::THIS_OBJECT, KEY, Some(id.0))
                        .unwrap()
                        .is_ok()
                );
            }
            assert!(owner.host_object(id).unwrap().has_upgrade_tag(KEY));
            assert!(!peer.host_object(peer_id).unwrap().has_upgrade_tag(KEY));
        },
    );
}
#[test]
fn native_named_and_ai_upgrade_routes_neither_publish_nor_steal_standalone_core_requests() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "native_named_and_ai_upgrade_routes_neither_publish_nor_steal_standalone_core_requests",
        || {
            let (mut owner, id) = world(true);
            let (mut peer, peer_id) = world(true);
            let _ = gamelogic::scripting::take_host_script_named_upgrade_requests();
            let _ = gamelogic::scripting::take_host_ai_player_build_upgrade_requests();
            gamelogic::scripting::request_host_script_named_upgrade(
                gamelogic::scripting::HostScriptNamedUpgradeRequest {
                    unit: ACTOR.into(),
                    upgrade: "ForeignStandaloneUpgrade".into(),
                },
            );
            gamelogic::scripting::request_host_ai_player_build_upgrade(
                "ForeignStandalonePlayer",
                "ForeignStandaloneResearch",
            );
            owner
                .with_owned_script_engine(|engine, receiving| {
                    let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                    engine
                        .friend_execute_action_with_driver_checked(
                            &action(ACTOR, KEY),
                            None,
                            ScriptContext::at_frame(0),
                            &mut driver,
                        )
                        .unwrap();
                })
                .unwrap();
            owner.apply_host_skirmish_script_requests();
            peer.apply_host_skirmish_script_requests();
            assert!(owner.host_object(id).unwrap().has_upgrade_tag(KEY));
            assert!(
                !owner
                    .host_object(id)
                    .unwrap()
                    .has_upgrade_tag("ForeignStandaloneUpgrade")
            );
            assert!(!peer.host_object(peer_id).unwrap().has_upgrade_tag(KEY));
            let named = gamelogic::scripting::take_host_script_named_upgrade_requests();
            assert_eq!(named.len(), 1);
            assert_eq!(named[0].upgrade, "ForeignStandaloneUpgrade");
            let research = gamelogic::scripting::take_host_ai_player_build_upgrade_requests();
            assert_eq!(
                research,
                [(
                    "ForeignStandalonePlayer".into(),
                    "ForeignStandaloneResearch".into()
                )]
            );
        },
    );
}

#[test]
fn immediate_replacement_uses_existing_exact_team_while_interpreter_is_held() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "immediate_replacement_uses_existing_exact_team_while_interpreter_is_held",
        || {
            let mut owner = GameLogic::new();
            owner.world_min = Vec3::ZERO;
            owner.world_max = Vec3::splat(1000.0);
            owner.add_player(Player::new(2, Team::USA, "ActualController", true));
            for name in ["FakeGLANamedLoan", "GLANamedLoan"] {
                let mut template = ThingTemplate::new(name);
                template.add_kind_of(KindOf::Structure).set_health(100.0);
                owner.templates.insert(name.into(), template);
            }
            let team_id = {
                let mut factory = owner.team_factory.lock().unwrap();
                factory
                    .init_team_owned(
                        "UpgradeReplacementTeam".into(),
                        "ActualController".into(),
                        false,
                        None,
                        Some(2),
                    )
                    .unwrap();
                let team = factory.create_team("UpgradeReplacementTeam").unwrap();
                let id = team.read().unwrap().get_id();
                id
            };
            let id = owner
                .create_object_on_exact_script_team_with_effects_checked(
                    "FakeGLANamedLoan",
                    2,
                    "UpgradeReplacementTeam",
                    team_id,
                    Vec3::new(100.0, 0.0, 100.0),
                    false,
                    None,
                    &mut Default::default(),
                )
                .unwrap()
                .unwrap();
            owner.host_object_mut(id).unwrap().name = ACTOR.into();
            owner.initialize_receiving_named_cache_for_fixture();
            let upgrade = "Upgrade_FakeGLABecomeReal";
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &owner, upgrade, "OBJECT", 0, 1,
            );
            owner
                .with_owned_script_engine(|engine, receiving| {
                    let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                    engine
                        .friend_execute_action_with_driver_checked(
                            &action(ACTOR, upgrade),
                            None,
                            ScriptContext::at_frame(0),
                            &mut driver,
                        )
                        .unwrap();
                    assert!(driver.world.owned_script_engine.is_none());
                    let replacement = driver
                        .world
                        .host_objects()
                        .registered_values()
                        .find(|object| object.template_name == "GLANamedLoan")
                        .expect("replacement");
                    assert_eq!(replacement.owner_player_id, Some(2));
                    assert_eq!(replacement.team_instance_id, Some(team_id));
                })
                .unwrap();
        },
    );
}
