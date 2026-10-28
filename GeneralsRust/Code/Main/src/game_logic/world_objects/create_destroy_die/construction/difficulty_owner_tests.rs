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

#[test]
fn ai_birth_uses_its_own_difficulty_factor_not_the_session_human_difficulty() {
    let mut world = GameLogic::new();
    world.game_mode = GameMode::SinglePlayer;
    world.admit_session_difficulty(2); // Human/session Hard.
    world
        .admit_combat_bonus_ini_sources(["GameData\nAISoloPlayerHealthBonus_Easy = 125%\nAISoloPlayerHealthBonus_Hard = 75%\nEnd\n"])
        .expect("AI factors belong to this session");
    world.add_player(Player::new(1, Team::China, "Easy AI", false));
    world.add_ai_opponent(1, Team::China, crate::ai::AIDifficulty::Easy);
    let mut template = ThingTemplate::new("OwnedAiDifficultyUnit");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert(template.name.clone(), template);

    let id = world
        .create_object_for_player("OwnedAiDifficultyUnit", 1, Vec3::ZERO)
        .expect("receiving AI unit should be created");

    assert_eq!(world.get_difficulty(), crate::ai::AIDifficulty::Hard);
    assert_eq!(
        world.host_ai_difficulty(1),
        Some(crate::ai::AIDifficulty::Easy)
    );
    assert_eq!(world.host_object(id).unwrap().health.maximum, 125.0);
}

#[test]
fn solo_health_factor_uses_exact_neutral_comparison_without_epsilon_fallback() {
    let mut neutral = world();
    neutral.admit_solo_player_health_bonus_for_difficulty([[1.0; 3]; 2]);
    let neutral_id = neutral
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        neutral.host_object(neutral_id).unwrap().health.maximum,
        100.0
    );

    let mut adjacent = world();
    let next_after_one = f32::from_bits(1.0_f32.to_bits() + 1);
    adjacent.admit_solo_player_health_bonus_for_difficulty([[next_after_one; 3]; 2]);
    let adjacent_id = adjacent
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        adjacent.host_object(adjacent_id).unwrap().health.maximum,
        100.0 * next_after_one
    );
    assert_ne!(
        adjacent.host_object(adjacent_id).unwrap().health.maximum,
        100.0
    );
}

#[test]
fn authored_solo_health_baselines_are_world_owned_across_same_ids_restore_and_reset() {
    fn admit_human_easy(world: &mut GameLogic, factor: &str) {
        world
            .admit_combat_bonus_ini_sources([format!(
                "GameData\nHumanSoloPlayerHealthBonus_Easy = {factor}\nEnd\n"
            )])
            .expect("application GameData factors should be admitted");
    }

    let mut left = world();
    let mut right = world();
    admit_human_easy(&mut left, "110%");
    admit_human_easy(&mut right, "140%");
    left.admit_combat_bonus_companion_sources(&[
        "GameData\nHumanSoloPlayerHealthBonus_Easy = 120%\nEnd\n".into(),
        "GameData\nHumanSoloPlayerHealthBonus_Easy = 125%\nEnd\n".into(),
    ])
    .expect("Map then Solo overrides should derive from this world's baseline");
    let left_id = left
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    let right_id = right
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(left_id, right_id);
    assert_eq!(left.host_object(left_id).unwrap().health.maximum, 125.0);
    assert_eq!(right.host_object(right_id).unwrap().health.maximum, 140.0);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&right).unwrap();
    let mut restored = world();
    admit_human_easy(&mut restored, "140%");
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let restored_id = restored
        .create_object_for_player("OwnedBonusStructure", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        restored.host_object(restored_id).unwrap().health.maximum,
        140.0
    );

    let baseline = left.combat_bonus_baseline().solo_health;
    left.restore_combat_bonus_baseline();
    assert_eq!(left.solo_player_health_bonus_for_difficulty, baseline);
    assert!(
        left.admit_combat_bonus_companion_sources(&[
            "GameData\nHumanSoloPlayerHealthBonus_Easy = 130%\nEnd\n".into(),
            "GameData\nHumanSoloPlayerHealthBonus_Easy = invalid\nEnd\n".into(),
        ])
        .is_err()
    );
    assert_eq!(left.solo_player_health_bonus_for_difficulty, baseline);
    left.reset();
    assert_eq!(left.solo_player_health_bonus_for_difficulty, baseline);
    assert_eq!(left.solo_player_health_bonus_for_difficulty[0][0], 1.1);
}
