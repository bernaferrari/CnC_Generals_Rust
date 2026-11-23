//! Actual script dispatch mutates only its supplied Main object owner.
use super::super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn execute(world: &mut GameLogic, kind: ScriptActionType, name: &str, value: Option<i32>) {
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    let mut action = ScriptAction::new(kind);
    if kind != ScriptActionType::DeleteAllUnmanned {
        let parameter = if kind == ScriptActionType::TeamSetUnmannedStatus {
            ParameterType::Team
        } else {
            ParameterType::Unit
        };
        action
            .add_parameter(Parameter::with_string(parameter, name.into()))
            .unwrap();
    }
    if let Some(value) = value {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    let context = gamelogic::scripting::executor::ScriptContext::at_frame(17);
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, &engine);
    engine.friend_execute_action_with_driver(&action, None, context, &mut driver);
}

#[test]
fn supplied_script_object_effects_are_synchronous_and_same_id_isolated() {
    let (mut owner, id, _) = world();
    let (peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    let name = owner.host_object(id).unwrap().name.clone();
    execute(
        &mut owner,
        ScriptActionType::DisableObjectSound,
        &name,
        None,
    );
    assert!(
        !owner
            .host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    assert!(
        peer.host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    execute(&mut owner, ScriptActionType::EnableObjectSound, &name, None);
    assert!(
        owner
            .host_object(id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    execute(
        &mut owner,
        ScriptActionType::NamedSetStealthEnabled,
        &name,
        Some(0),
    );
    assert!(owner.host_object(id).unwrap().script_unstealthed);
    assert!(!peer.host_object(id).unwrap().script_unstealthed);
    execute(
        &mut owner,
        ScriptActionType::ObjectCreateRadarEvent,
        &name,
        Some(4),
    );
    assert!(owner.radar().get_last_event_loc().is_some());
    assert!(peer.radar().get_last_event_loc().is_none());
    execute(
        &mut owner,
        ScriptActionType::NamedSetUnmannedStatus,
        &name,
        None,
    );
    assert!(owner.host_object(id).unwrap().status.disabled_unmanned);
    assert!(!peer.host_object(id).unwrap().status.disabled_unmanned);
    assert_eq!(owner.host_object(id).unwrap().team, Team::Neutral);
    execute(&mut owner, ScriptActionType::DeleteAllUnmanned, "", None);
    assert!(
        owner.host_object(id).unwrap().status.destroyed
            || !owner.host_object(id).unwrap().is_alive()
    );
    assert!(peer.host_object(id).unwrap().is_alive());
}

fn unmanned_world() -> (GameLogic, ObjectId, ObjectId, u32) {
    use super::super::named_command_test_support::world_with;
    use crate::fixture_content::FixtureObject;
    use crate::game_logic::game_logic::PlayerSideRole;

    let (mut world, infantry, _) = world_with(
        &[FixtureObject::new("NamedPowerVehicle")
            .kind_of("VEHICLE SELECTABLE")
            .health(100.0)
            .energy(20)],
        &[],
        "",
    );
    let mut neutral = Player::new(9, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    world.add_player(neutral);
    let neutral_team = world.host_default_team_id(9).expect("registered neutral default Team");
    let id = world
        .create_object_for_player("NamedPowerVehicle", 1, Vec3::new(110.0, 0.0, 110.0))
        .unwrap();
    assert!(world.set_host_object_name(id, "NamedPowerTarget"));
    assert!(world.add_receiving_named_script_object(id));
    assert_eq!(world.host_object_id_by_script_name("NamedPowerTarget"), Some(id));
    (world, id, infantry, neutral_team)
}

#[test]
fn named_unmanned_releases_power_before_neutral_capture_in_the_receiving_world() {
    let (mut owner, id, _, neutral_team) = unmanned_world();
    let (peer, peer_id, _, _) = unmanned_world();
    assert_eq!(id, peer_id);
    assert_eq!(owner.get_player(1).unwrap().power_produced, 20);

    execute(&mut owner, ScriptActionType::NamedSetUnmannedStatus, "NamedPowerTarget", None);

    let vehicle = owner.host_object(id).unwrap();
    assert!(vehicle.status.disabled_unmanned);
    assert_eq!(owner.get_player(1).unwrap().power_produced, 0);
    assert_eq!(vehicle.owner_player_id, Some(9));
    assert_eq!(vehicle.team_instance_id, Some(neutral_team));
    assert_eq!(owner.get_player(9).unwrap().power_produced, 0);
    assert_eq!(peer.get_player(1).unwrap().power_produced, 20);
    assert_eq!(peer.host_object(peer_id).unwrap().owner_player_id, Some(1));
    assert!(!peer.host_object(peer_id).unwrap().status.disabled_unmanned);
}

#[test]
fn team_unmanned_visits_each_original_member_as_capture_changes_the_team() {
    let (mut owner, vehicle, infantry, neutral_team) = unmanned_world();
    let (peer, peer_vehicle, peer_infantry, _) = unmanned_world();
    assert_eq!((vehicle, infantry), (peer_vehicle, peer_infantry));
    let team = owner.host_object(vehicle).unwrap().team_instance_name.clone();
    owner.apply_player_drawable_selection(1, &[vehicle, infantry], true);
    assert_eq!(owner.get_player(1).unwrap().selected_objects.len(), 2);

    execute(&mut owner, ScriptActionType::TeamSetUnmannedStatus, &team, None);

    for id in [vehicle, infantry] {
        let object = owner.host_object(id).unwrap();
        assert!(object.is_unmanned());
        assert_eq!(object.owner_player_id, Some(9));
        assert_eq!(object.team_instance_id, Some(neutral_team));
        assert!(!object.selected);
        assert!(!peer.host_object(id).unwrap().is_unmanned());
        assert_eq!(peer.host_object(id).unwrap().owner_player_id, Some(1));
    }
    assert_eq!(owner.get_player(1).unwrap().power_produced, 0);
    assert!(owner.get_player(1).unwrap().selected_objects.is_empty());
    assert_eq!(peer.get_player(1).unwrap().power_produced, 20);
}
