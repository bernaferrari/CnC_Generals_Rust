use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

const TARGET: &str = "BoobyStatusOwnerTarget";

fn world() -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "BoobyStatusOwner.ini".into(),
        format!(
            "Object {TARGET}\n KindOf = STRUCTURE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ),
    )])
    .expect("receiving Object INI admitted");
    game.start_new_game(GameMode::SinglePlayer);
    game.add_player(
        Player::new(3, Team::GLA, "TrapOwner", false).with_script_identity("TrapOwner"),
    );
    let id = game
        .create_object_for_player(TARGET, 3, Vec3::ZERO)
        .expect("receiving target");
    (game, id)
}

fn script_observes_trap(game: &mut GameLogic, id: ObjectId) -> bool {
    use gamelogic::common::ObjectStatusMaskType;
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    game.host_object_mut(id).unwrap().name = "TrapTarget".into();
    let mut condition = Condition::new(ConditionType::UnitHasObjectStatus);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TrapTarget".into(),
        ))
        .unwrap();
    let mut status = Parameter::with_string(ParameterType::ObjectStatus, "BOOBY_TRAPPED".into());
    status.object_status = ObjectStatusMaskType::from_case_insensitive_name("BOOBY_TRAPPED")
        .expect("authored status name");
    condition.add_parameter(status).unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "TrapOwner".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveTrap".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    let before = game.get_player(3).unwrap().resources.supplies;
    game.update_local_script_engine_with_owner(&engine).unwrap();
    game.get_player(3).unwrap().resources.supplies == before + 1
}

#[test]
fn named_script_observes_each_worlds_canonical_trap_status() {
    let (mut owner, owner_id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(owner_id, peer_id);
    owner
        .host_object_mut(owner_id)
        .unwrap()
        .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
    assert!(script_observes_trap(&mut owner, owner_id));
    assert!(!script_observes_trap(&mut peer, peer_id));
    owner
        .host_object_mut(owner_id)
        .unwrap()
        .apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
    assert!(!script_observes_trap(&mut owner, owner_id));
}

#[test]
fn status_mask_mutation_interleaves_by_world_and_survives_encoded_save_restore() {
    let (mut first, first_id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(first_id, peer_id, "worlds reuse ObjectIds");

    first
        .host_object_mut(first_id)
        .expect("first target")
        .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
    assert!(
        first
            .host_object(first_id)
            .expect("first target")
            .status
            .booby_trapped
    );
    assert!(
        first
            .host_object(first_id)
            .expect("first target")
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        !peer
            .host_object(peer_id)
            .expect("peer target")
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    peer.host_object_mut(peer_id)
        .expect("peer target")
        .apply_status_bits_upgrade_masks(&["BOOBY_TRAPPED"], &[]);
    peer.host_object_mut(peer_id)
        .expect("peer target")
        .apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
    assert!(
        first
            .host_object(first_id)
            .expect("first target remains trapped")
            .status
            .booby_trapped
    );
    assert!(
        !peer
            .host_object(peer_id)
            .expect("peer target cleared")
            .status
            .booby_trapped
    );

    let directory = tempfile::tempdir().expect("save directory");
    let mut saves =
        generals_main::save_load::SaveFileManager::with_save_directory(directory.path());
    saves.init().expect("initialize save manager");
    let info = generals_main::save_load::SaveGameInfo {
        pristine_map_name: None,
        filename: "booby-status-owner".into(),
        display_name: "Booby status owner continuation".into(),
        description: String::new(),
        map_name: "BoobyStatusOwnerFixture".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: generals_main::save_load::GameDifficulty::Medium,
        save_type: generals_main::save_load::SaveFileType::Normal,
    };
    saves
        .save_game("booby-status-owner", &first, &info)
        .expect("save trapped world");
    let (mut restored, _) = world();
    saves
        .load_game("booby-status-owner", &mut restored)
        .expect("restore trapped world");
    let loaded = restored.host_object(first_id).expect("restored target");
    assert!(loaded.status.booby_trapped);
    assert!(loaded.has_object_status_bit("BOOBY_TRAPPED"));

    restored
        .host_object_mut(first_id)
        .expect("restored target")
        .apply_status_bits_upgrade_masks(&[], &["BOOBY_TRAPPED"]);
    let continued = restored.host_object(first_id).expect("continued target");
    assert!(!continued.status.booby_trapped);
    assert!(!continued.has_object_status_bit("BOOBY_TRAPPED"));
}
