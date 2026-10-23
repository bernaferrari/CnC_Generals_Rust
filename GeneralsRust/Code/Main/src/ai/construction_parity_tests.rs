use super::{AIDifficulty, AIPlayer};
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn automatic_power_override_preserves_priority_and_scaffold_rules() {
    // AISkirmishPlayer.cpp:183-236: ordinary automatic pads cannot select
    // themselves, but an automatic FS_POWER plan overrides a priority plan
    // until another power plant is under construction.
    let mut logic = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "AI", false);
    player.resources.supplies = 10_000;
    logic.add_player(player);
    for (name, power) in [("PriorityPlan", false), ("PowerPlan", true)] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(KindOf::Structure).set_cost(100, 0);
        if power {
            template.add_kind_of(KindOf::FSPower);
        }
        logic.templates.insert(name.into(), template);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("PriorityPlan", Vec3::new(100.0, 0.0, 100.0), 1);
    ai.add_layout_building("PowerPlan", Vec3::new(200.0, 0.0, 100.0), 1);
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, false),
        Some(1)
    );

    ai.building_queue[1].automatic_build = false;
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, false),
        Some(0)
    );
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, true),
        Some(1)
    );

    let position = Vec3::new(300.0, 0.0, 100.0);
    let scaffold = logic
        .create_object_under_construction("PowerPlan", Team::USA, position)
        .expect("power scaffold");
    ai.add_layout_building("PowerPlan", position, 1);
    ai.building_queue[2].object_id = Some(scaffold);
    ai.building_queue[1].automatic_build = true;
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, true),
        Some(0)
    );
}

#[test]
fn placement_retains_cpp_delayed_second_edge_result() {
    // AIPlayer.cpp:554-561 checks the bottom result only after the row loop.
    // Success at its first bottom sample is overwritten by the next top test;
    // success at its final bottom sample survives. The right edge is identical.
    for (query_to_accept, survives) in [(0, true), (1, false), (3, true), (5, false), (7, true)] {
        let queries = std::cell::Cell::new(0);
        let found = AIPlayer::wiggle_find_legal_build_position(Vec3::ZERO, |_| {
            let index = queries.get();
            queries.set(index + 1);
            index == query_to_accept
        });
        assert_eq!(found.is_some(), survives, "query {query_to_accept}");
        if !survives {
            assert_eq!(queries.get(), 3720, "search must exhaust all rings");
        }
    }
}

fn energy_admission_world() -> GameLogic {
    let source = r#"
Object AdmissionPower
 KindOf = STRUCTURE FS_POWER
 EnergyProduction = 20
 BuildCost = 100
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object AdmissionSink
 KindOf = STRUCTURE POWERED
 EnergyProduction = -10
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object AdmissionZero
 KindOf = STRUCTURE
 EnergyProduction = 0
End
Object AdmissionOne
 KindOf = STRUCTURE FS_POWER
 EnergyProduction = 1
End
Object BadEnergyChild
 KindOf = INFANTRY
 Body = ActiveBody BodyTag
 MaxHealth = NaN
 InitialHealth = 100
 End
End
Object AdmissionPoweredProducer
 KindOf = STRUCTURE POWERED FS_POWER
 EnergyProduction = 5
 Body = ActiveBody BodyTag
 MaxHealth = 100
 InitialHealth = 100
 End
End
Object AdmissionHeavySink
 KindOf = STRUCTURE POWERED
 EnergyProduction = -100
 Body = ActiveBody BodyTag
 MaxHealth = 100
 InitialHealth = 100
 End
End
"#;
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "energy-admission.ini".to_owned(),
                source.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    for id in [1, 2] {
        let mut player = Player::new(
            id,
            Team::USA,
            if id == 1 {
                "ExactEnergy"
            } else {
                "ExactEnergyPeer"
            },
            id == 1,
        );
        player.map_side.map_player_name = if id == 1 {
            "ExactEnergy"
        } else {
            "ExactEnergyPeer"
        }
        .into();
        player.resources.supplies = 10_000;
        player.add_radar(false);
        world.add_player(player);
    }
    world
}
#[test]
fn actual_factory_energy_is_visible_to_hud_ai_and_radar_before_any_periodic_tick() {
    let mut world = energy_admission_world();
    let mut peer = energy_admission_world();
    let sink = world
        .create_object_for_player("AdmissionSink", 1, Vec3::ZERO)
        .unwrap();
    let peer_sink = peer
        .create_object_for_player("AdmissionSink", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(sink, peer_sink);
    assert!(
        !world
            .get_player(1)
            .unwrap()
            .has_sufficient_energy_at(world.get_frame())
    );
    assert!(
        world
            .host_object(sink)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert!(!world.get_player(1).unwrap().has_radar());
    assert!(
        world.get_player(2).unwrap().has_radar(),
        "same-faction other owner is untouched"
    );
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    let mut ordinary = ThingTemplate::new("EnergyPriority");
    ordinary.add_kind_of(KindOf::Structure).set_cost(100, 0);
    world.templates.insert("EnergyPriority".into(), ordinary);
    // Seed the real receiving power template through a scaffold; its initial
    // UNDER_CONSTRUCTION membership must not contribute production.
    world
        .create_object_under_construction_for_player(
            "AdmissionPower",
            1,
            Vec3::new(300., 0., 300.),
            0.,
        )
        .unwrap();
    ai.add_building("EnergyPriority", Vec3::new(100., 0., 100.), 1);
    ai.add_layout_building("AdmissionPower", Vec3::new(200., 0., 100.), 1);
    ai.building_queue[1].automatic_build = false;
    let brownout = !world
        .get_player(1)
        .unwrap()
        .has_sufficient_energy_at(world.get_frame());
    assert_eq!(
        ai.select_priority_or_power_build(&world, 0., brownout),
        Some(1)
    );
    let frame = world.get_frame();
    let plant = world
        .create_object_for_player("AdmissionPower", 1, Vec3::new(50., 0., 50.))
        .unwrap();
    assert_ne!(plant, sink);
    assert_eq!(world.get_frame(), frame);
    let player = world.get_player(1).unwrap();
    assert_eq!((player.power_produced, player.power_consumed), (20, 10));
    assert!(player.has_sufficient_energy_at(frame) && player.has_radar());
    assert!(
        !world
            .host_object(sink)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert_eq!(
        ai.select_priority_or_power_build(&world, 0., !player.has_sufficient_energy_at(frame)),
        Some(0)
    );
    let hud = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    assert_eq!(
        (
            hud.local_power,
            hud.local_power_produced,
            hud.local_power_consumed
        ),
        (10, 20, 10)
    );
    assert_eq!(world.get_frame(), frame);
    assert_eq!(
        (
            peer.get_player(1).unwrap().power_produced,
            peer.get_player(1).unwrap().power_consumed
        ),
        (0, 10)
    );
    assert!(!peer.get_player(1).unwrap().has_radar());
    assert!(
        peer.host_object(peer_sink)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert_eq!(world.get_player(2).unwrap().power_produced, 0);
}
#[test]
fn nonzero_factory_energy_repeats_brownout_callback_but_zero_and_scaffold_do_not() {
    let mut world = energy_admission_world();
    world
        .create_object_for_player("AdmissionPower", 1, Vec3::ZERO)
        .unwrap();
    world.get_player_mut(1).unwrap().disable_radar();
    world
        .create_object_for_player("AdmissionZero", 1, Vec3::ZERO)
        .unwrap();
    world
        .create_object_under_construction_for_player("AdmissionPower", 1, Vec3::ZERO, 0.)
        .unwrap();
    assert!(!world.get_player(1).unwrap().has_radar());
    assert_eq!(world.get_player(1).unwrap().power_produced, 20);
    world
        .create_object_for_player("AdmissionOne", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(world.get_player(1).unwrap().power_produced, 21);
    assert!(
        world.get_player(1).unwrap().has_radar(),
        "nonzero Energy callback executes even without sufficiency edge"
    );
}
#[test]
fn explicit_admission_excludes_disabled_positive_but_retains_disabled_signed_consumer() {
    let mut world = energy_admission_world();
    let scaffold = world
        .create_object_under_construction_for_player("AdmissionPower", 1, Vec3::ZERO, 0.)
        .unwrap();
    let template = world
        .host_object(scaffold)
        .unwrap()
        .thing()
        .template
        .clone();
    let mut disabled =
        crate::game_logic::Object::new(template, crate::game_logic::ObjectId(900), Team::USA);
    disabled.owner_player_id = Some(1);
    disabled.status.disabled_emp = true;
    world.try_add_object(disabled).unwrap();
    assert_eq!(world.get_player(1).unwrap().power_produced, 0);
    let sink = world
        .create_object_for_player("AdmissionSink", 2, Vec3::ZERO)
        .unwrap();
    let template = world.host_object(sink).unwrap().thing().template.clone();
    let mut disabled =
        crate::game_logic::Object::new(template, crate::game_logic::ObjectId(901), Team::USA);
    disabled.owner_player_id = Some(1);
    disabled.status.disabled_emp = true;
    world.try_add_object(disabled).unwrap();
    assert_eq!(
        (
            world.get_player(1).unwrap().power_produced,
            world.get_player(1).unwrap().power_consumed
        ),
        (0, 10)
    );
    assert_eq!(world.get_player(2).unwrap().power_consumed, 10);
    assert!(
        world
            .host_object(sink)
            .unwrap()
            .status
            .disabled_underpowered,
        "second actual Team owner receives its birth callback"
    );
    assert!(!world.get_player(1).unwrap().has_radar());
}

#[test]
fn actual_positive_powered_producers_recurse_in_player_prototype_team_member_order() {
    let mut world = energy_admission_world();
    let mut peer = energy_admission_world();
    world
        .create_object_for_player("AdmissionPower", 1, Vec3::ZERO)
        .unwrap();
    peer.create_object_for_player("AdmissionPower", 1, Vec3::ZERO)
        .unwrap();
    fn instance(world: &mut GameLogic, name: &str) -> u32 {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(name.into(), "ExactEnergy".into(), false, None, Some(1))
            .unwrap();
        let (team, effects) = factory
            .create_inactive_team_with_owned_effects(name)
            .unwrap();
        assert!(effects.is_empty());
        let id = team.read().unwrap().get_id();
        id
    }
    let first = instance(&mut world, "ZuluFirst");
    let second = instance(&mut world, "AlphaSecond");
    let create = |world: &mut GameLogic, name: &str, team: u32| {
        world
            .create_object_on_exact_script_team_with_effects_checked(
                "AdmissionPoweredProducer",
                1,
                name,
                team,
                Vec3::ZERO,
                false,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap()
    };
    let a1 = create(&mut world, "ZuluFirst", first);
    let b1 = create(&mut world, "AlphaSecond", second);
    let a2 = create(&mut world, "ZuluFirst", first);
    let newest = {
        let mut factory = world.team_factory.lock().unwrap();
        let (team, effects) = factory
            .create_inactive_team_with_owned_effects("ZuluFirst")
            .unwrap();
        assert!(effects.is_empty());
        let identity = team.read().unwrap().get_id();
        identity
    };
    let a3 = create(&mut world, "ZuluFirst", newest);
    assert_eq!(world.get_player(1).unwrap().power_produced, 40);
    assert!(
        [a1, b1, a2, a3].iter().all(|id| !world
            .host_object(*id)
            .unwrap()
            .status
            .disabled_underpowered)
    );
    world.health_events.take_disable_audio();
    let sink = world
        .create_object_for_player("AdmissionHeavySink", 1, Vec3::ZERO)
        .unwrap();
    let callbacks = world
        .health_events
        .take_disable_audio()
        .into_iter()
        .map(|event| event.object)
        .collect::<Vec<_>>();
    assert_eq!(
        callbacks,
        vec![sink, a3, a2, a1, b1],
        "prototype admission then per-Team prepend members, not global registration"
    );
    assert_eq!(
        (
            world.get_player(1).unwrap().power_produced,
            world.get_player(1).unwrap().power_consumed
        ),
        (20, 100)
    );
    assert!(
        [a1, b1, a2, a3, sink].iter().all(|id| world
            .host_object(*id)
            .unwrap()
            .status
            .disabled_underpowered)
    );
    assert!(!world.get_player(1).unwrap().has_radar());
    assert_eq!(
        (
            peer.get_player(1).unwrap().power_produced,
            peer.get_player(1).unwrap().power_consumed
        ),
        (20, 0)
    );
    assert!(peer.get_player(1).unwrap().has_radar());
}

fn prepare_scripted_external_energy(
    world: &mut GameLogic,
    fail: bool,
) -> crate::game_logic::Object {
    prepare_scripted_external_energy_with_nested(world, fail, false)
}
fn prepare_scripted_external_energy_with_nested(
    world: &mut GameLogic,
    fail: bool,
    nested: bool,
) -> crate::game_logic::Object {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    let scaffold = world
        .create_object_under_construction_for_player("AdmissionPower", 1, Vec3::ZERO, 0.)
        .unwrap();
    let mut factory = world.team_factory.lock().unwrap();
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "OnExternalEnergyCreate");
    factory
        .init_team_owned(
            "ExternalEnergyTeam".into(),
            "ExactEnergy".into(),
            false,
            Some(&dict),
            Some(1),
        )
        .unwrap();
    factory
        .init_team_owned(
            "ExternalChildTeam".into(),
            "ExactEnergy".into(),
            false,
            None,
            Some(1),
        )
        .unwrap();
    drop(factory);
    let mut count = ScriptAction::new(ScriptActionType::IncrementCounter);
    count
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    count
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "ExternalEnergyVisits".into(),
        ))
        .unwrap();
    if fail || nested {
        let mut create = ScriptAction::new(ScriptActionType::CreateObject);
        for parameter in [
            Parameter::with_string(
                ParameterType::ObjectType,
                (if fail {
                    "BadEnergyChild"
                } else {
                    "AdmissionOne"
                })
                .into(),
            ),
            Parameter::with_string(
                ParameterType::Team,
                (if fail {
                    "ExternalChildTeam"
                } else {
                    "ExternalEnergyTeam"
                })
                .into(),
            ),
            Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(10., 20., 0.),
            ),
            Parameter::with_real(ParameterType::Angle, 0.),
        ] {
            create.add_parameter(parameter).unwrap();
        }
        count.next_action = Some(Box::new(create));
    }
    let mut script = Script::new();
    script.script_name = "OnExternalEnergyCreate".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(count));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine.set_counter("ExternalEnergyVisits", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(engine);
    let mut incoming = crate::game_logic::Object::new(
        world
            .host_object(scaffold)
            .unwrap()
            .thing()
            .template
            .clone(),
        crate::game_logic::ObjectId(990),
        Team::USA,
    );
    incoming.owner_player_id = Some(1);
    incoming.team_instance_name = "ExternalEnergyTeam".into();
    incoming
}
#[test]
fn scripted_external_energy_birth_executes_actual_owned_team_actions_before_member_commit() {
    let mut world = energy_admission_world();
    let peer = energy_admission_world();
    let incoming = prepare_scripted_external_energy(&mut world, false);
    let id = world.try_add_object(incoming).unwrap();
    assert_eq!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter("ExternalEnergyVisits")
            .unwrap()
            .value,
        1
    );
    let actor = world.host_object(id).unwrap();
    let team_id = actor.team_instance_id.unwrap();
    assert_eq!(actor.team_instance_name, "ExternalEnergyTeam");
    assert_eq!(
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(team_id)
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        [id.0]
    );
    assert_eq!(world.get_player(1).unwrap().power_produced, 20);
    assert!(world.get_player(1).unwrap().has_radar());
    assert_eq!(peer.get_player(1).unwrap().power_produced, 0);
    assert!(peer.host_object(id).is_none());
}
#[test]
fn failed_actual_team_script_retains_partial_script_team_owner_without_publishing_incoming_life() {
    let mut world = energy_admission_world();
    let peer = energy_admission_world();
    let incoming = prepare_scripted_external_energy(&mut world, true);
    let before = world.host_objects().len();
    let error = world.try_add_object(incoming).unwrap_err();
    assert!(
        error.to_string().contains("BadEnergyChild") && error.to_string().contains("MaxHealth")
    );
    assert!(
        world
            .host_object(crate::game_logic::ObjectId(990))
            .is_none()
    );
    assert_eq!(
        world.host_objects().len(),
        before,
        "failed child Body is rejected before its own birth too"
    );
    assert_eq!(world.get_player(1).unwrap().power_produced, 0);
    assert_eq!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter("ExternalEnergyVisits")
            .unwrap()
            .value,
        1,
        "executed script side effect is retained, not falsely rolled back"
    );
    let factory = world.team_factory.lock().unwrap();
    let teams = factory
        .get_all_teams()
        .into_iter()
        .filter(|team| team.read().unwrap().get_name().as_str() == "ExternalEnergyTeam")
        .collect::<Vec<_>>();
    assert_eq!(teams.len(), 1);
    assert!(
        teams[0].read().unwrap().get_members().is_empty(),
        "actual partial Team remains owned by receiving factory, no incoming actor stranded"
    );
    drop(factory);
    let template = world
        .host_objects()
        .values()
        .next()
        .unwrap()
        .thing()
        .template
        .clone();
    let mut retry =
        crate::game_logic::Object::new(template, crate::game_logic::ObjectId(990), Team::USA);
    retry.owner_player_id = Some(1);
    retry.team_instance_name = "ExternalEnergyTeam".into();
    let retry_error = world.try_add_object(retry).unwrap_err();
    assert_eq!(retry_error.to_string(), error.to_string());
    assert_eq!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter("ExternalEnergyVisits")
            .unwrap()
            .value,
        1,
        "failed exact Team never replays or silently becomes Ready"
    );
    assert!(
        world
            .host_object(crate::game_logic::ObjectId(990))
            .is_none()
    );
    assert!(world.team_creation_error().is_some());
    let snapshot_error = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap_err();
    assert!(snapshot_error.to_string().contains("creation failed"));
    assert!(peer.host_object(crate::game_logic::ObjectId(990)).is_none());
    assert_eq!(peer.get_player(1).unwrap().power_produced, 0);
}
#[test]
fn external_energy_missing_interpreter_rejects_before_actual_team_creation() {
    let mut world = energy_admission_world();
    let incoming = prepare_scripted_external_energy(&mut world, false);
    let engine = world.owned_script_engine.take().unwrap();
    let error = world.try_add_object(incoming).unwrap_err();
    assert!(error.to_string().contains("driving interpreter"));
    assert!(
        world
            .host_object(crate::game_logic::ObjectId(990))
            .is_none()
    );
    assert_eq!(world.get_player(1).unwrap().power_produced, 0);
    assert_eq!(engine.get_counter("ExternalEnergyVisits").unwrap().value, 0);
    let factory = world.team_factory.lock().unwrap();
    assert!(
        !factory
            .get_all_teams()
            .iter()
            .any(|team| team.read().unwrap().get_name().as_str() == "ExternalEnergyTeam")
    );
}

#[test]
fn scripted_external_energy_allows_real_nested_member_while_creation_is_running() {
    let mut world = energy_admission_world();
    let incoming = prepare_scripted_external_energy_with_nested(&mut world, false, true);
    let id = world.try_add_object(incoming).unwrap();
    let team_id = world.host_object(id).unwrap().team_instance_id.unwrap();
    let team = world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    let team = team.read().unwrap();
    assert_eq!(
        team.get_members().len(),
        2,
        "actual nested factory birth joins existing Running Team"
    );
    assert!(team.get_members().contains(&id.0));
    assert!(!team.creation_is_running());
    assert!(team.creation_failure().is_none());
    drop(team);
    assert_eq!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter("ExternalEnergyVisits")
            .unwrap()
            .value,
        1
    );
    assert_eq!(world.get_player(1).unwrap().power_produced, 21);
    assert!(
        crate::save_load::SnapshotBuilder::new()
            .create_world_snapshot(&world)
            .is_ok()
    );
}
#[test]
fn running_actual_team_creation_cannot_be_snapshot_as_ready() {
    let mut world = energy_admission_world();
    let incoming = prepare_scripted_external_energy(&mut world, false);
    let id = world.try_add_object(incoming).unwrap();
    let team_id = world.host_object(id).unwrap().team_instance_id.unwrap();
    let team = world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    team.write().unwrap().begin_creation_actions();
    let error = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap_err();
    assert!(error.to_string().contains("running creation actions"));
    assert!(team.read().unwrap().creation_is_running());
    assert!(world.host_object(id).is_some());
}

#[test]
fn actual_singleton_factory_reuses_running_identity_and_executes_its_creation_actions() {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    let mut world = energy_admission_world();
    let peer = energy_admission_world();
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "SingletonCreate");
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        let (_, initial) = factory
            .init_team_owned_with_effects(
                "ActualSingleton".into(),
                "ExactEnergy".into(),
                true,
                Some(&dict),
                Some(1),
            )
            .unwrap();
        assert_eq!(initial, ["SingletonCreate"]);
        factory
            .find_team_instances("ActualSingleton")
            .into_iter()
            .next()
            .unwrap()
    };
    let mut increment = ScriptAction::new(ScriptActionType::IncrementCounter);
    increment
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    increment
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "SingletonVisits".into(),
        ))
        .unwrap();
    let mut on_create = Script::new();
    on_create.script_name = "SingletonCreate".into();
    on_create.is_subroutine = true;
    on_create.action = Some(Box::new(increment));
    let mut list = ScriptList::new();
    list.append_script(Box::new(on_create));
    let mut interpreter = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    interpreter.set_counter("SingletonVisits", 0).unwrap();
    interpreter
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(interpreter);
    // Establish the canonical outer continuation. This test covers the shared
    // receipt and actual factory/action primitives, not full recursive script
    // dispatch (the current interpreter separately rejects active subroutines).
    team.write().unwrap().begin_creation_actions();
    let (nested, actions) = world
        .team_factory
        .lock()
        .unwrap()
        .create_inactive_team_with_owned_effects("ActualSingleton")
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&nested, &team));
    assert_eq!(actions, ["SingletonCreate"]);
    world
        .run_owned_team_creation_actions(&nested, actions, None, &mut Default::default())
        .unwrap();
    assert_eq!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter("SingletonVisits")
            .unwrap()
            .value,
        1
    );
    assert!(
        team.read().unwrap().creation_is_running(),
        "nested finish cannot clear the still-running outer phase"
    );
    team.write().unwrap().finish_creation_actions(Ok(()));
    assert!(!team.read().unwrap().creation_is_running());
    assert!(team.read().unwrap().creation_failure().is_none());
    assert_eq!(
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_instances("ActualSingleton")
            .len(),
        1
    );
    assert!(world.team_creation_error().is_none());
    assert_eq!(peer.get_player(1).unwrap().power_produced, 0);
}
