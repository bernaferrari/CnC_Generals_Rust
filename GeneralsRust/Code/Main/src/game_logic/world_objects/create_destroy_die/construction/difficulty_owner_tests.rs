//! CPP ScriptActions difficulty policy affects existing objects immediately and
//! each later Object::init, including creation while the interpreter is borrowed.
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.game_mode = GameMode::SinglePlayer;
    world.admit_session_difficulty(0);
    world.add_player(Player::new(0, Team::USA, "BonusOwner", true));
    let mut template = ThingTemplate::new("OwnedBonusStructure");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let mut factory = world.team_factory.lock().unwrap();
    factory
        .init_team_owned(
            "BonusTeam".into(),
            "BonusOwner".into(),
            false,
            None,
            Some(0),
        )
        .unwrap();
    factory
        .create_inactive_team_with_owned_effects("BonusTeam")
        .unwrap()
        .0
        .write()
        .unwrap()
        .set_active();
    drop(factory);
    world
}
fn toggle(enable: bool) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::ObjectAllowBonuses);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, i32::from(enable)))
        .unwrap();
    action
}
fn spawn() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "OwnedBonusStructure".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "BonusTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::scripting::core::Coord3D::new(10.0, 10.0, 0.0),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Angle, 0.0))
        .unwrap();
    action
}

#[test]
fn detached_interpreter_toggle_changes_same_action_chain_birth_and_only_its_owner() {
    let mut owner = world();
    let mut peer = world();
    let existing = owner
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert!(
        owner
            .host_object(existing)
            .unwrap()
            .is_receiving_difficulty_bonus
    );
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .is_receiving_difficulty_bonus
    );
    let mut action = toggle(false);
    action.next_action = Some(Box::new(spawn()));
    owner
        .with_owned_script_engine(|engine, world| {
            assert!(world.owned_script_engine.is_none());
            world.execute_script_team_actions(engine, &action, None);
        })
        .unwrap();
    assert!(!owner.objects_receive_difficulty_bonus);
    assert_eq!(owner.host_objects().registered_len(), 2);
    assert!(
        owner
            .host_objects()
            .registered_values()
            .all(|object| !object.is_receiving_difficulty_bonus)
    );
    assert!(peer.objects_receive_difficulty_bonus);
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .is_receiving_difficulty_bonus
    );
    owner
        .with_owned_script_engine(|engine, world| {
            world.execute_script_team_actions(engine, &toggle(true), None);
        })
        .unwrap();
    assert!(
        owner
            .host_objects()
            .registered_values()
            .all(|object| object.is_receiving_difficulty_bonus)
    );
}

#[test]
fn owned_birth_policy_survives_real_snapshot_and_controls_next_factory_admission() {
    let mut owner = world();
    owner.set_owned_object_difficulty_bonus(false);
    let first = owner
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert!(
        !owner
            .host_object(first)
            .unwrap()
            .is_receiving_difficulty_bonus
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut restored = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(!restored.objects_receive_difficulty_bonus);
    let next = restored
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert!(
        !restored
            .host_object(next)
            .unwrap()
            .is_receiving_difficulty_bonus
    );
    restored.reset();
    assert!(restored.objects_receive_difficulty_bonus);
}

#[test]
fn interpreter_admission_adopts_policy_once_without_tick_global_polling() {
    let mut world = world();
    let engine = ScriptEngine::new().unwrap();
    engine.set_objects_should_receive_difficulty_bonus(false);
    world.install_owned_script_engine(engine);
    let id = world
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert!(!world.host_object(id).unwrap().is_receiving_difficulty_bonus);
}

#[test]
fn actual_birth_and_save_use_selected_session_difficulty_independently_of_peer() {
    let mut easy = world();
    let mut hard = world();
    hard.admit_session_difficulty(2);
    let easy_id = easy
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    let hard_id = hard
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(easy.get_difficulty(), crate::ai::AIDifficulty::Easy);
    assert_eq!(hard.get_difficulty(), crate::ai::AIDifficulty::Hard);
    assert_eq!(
        easy.host_object(easy_id).unwrap().weapon_bonus_solo,
        crate::game_logic::host_faction_skirmish_residual::solo_weapon_bonus_condition(
            true,
            crate::ai::AIDifficulty::Easy
        )
    );
    assert_eq!(
        hard.host_object(hard_id).unwrap().weapon_bonus_solo,
        crate::game_logic::host_faction_skirmish_residual::solo_weapon_bonus_condition(
            true,
            crate::ai::AIDifficulty::Hard
        )
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&hard).unwrap();
    let mut restored = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.get_difficulty(), crate::ai::AIDifficulty::Hard);
    let next = restored
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        restored.host_object(next).unwrap().weapon_bonus_solo,
        hard.host_object(hard_id).unwrap().weapon_bonus_solo
    );
    assert_eq!(easy.get_difficulty(), crate::ai::AIDifficulty::Easy);
}
