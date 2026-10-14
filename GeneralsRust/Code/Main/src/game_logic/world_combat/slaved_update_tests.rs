//! Actual receiving SlavedUpdate callback order (CPP SlavedUpdate.cpp:91–261,410–674).
use crate::game_logic::host_slave_drones::BattleDroneRepairState as Repair;
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;
fn world() -> (
    GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "owner", true));
    let mut master = ThingTemplate::new("OwnedSlaveMaster");
    master
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game.templates.insert(master.name.clone(), master);
    // Actual authored locomotor + SlavedUpdate factory path; a mobile KindOf
    // alone is not CPP getCurLocomotor admission (SlavedUpdate.cpp:109–119).
    let text = r#"
Object OwnedSlaveRepair
 KindOf = VEHICLE ATTACKABLE
 Locomotor = SET_NORMAL ComancheLocomotor
 Behavior = AIUpdateInterface OwnedSlaveAI
  AutoAcquireEnemiesWhenIdle = Yes
 End
 Behavior = SlavedUpdate OwnedSlaveUpdate
  GuardMaxRange = 40
  RepairRatePerSecond = 30
  RepairWhenHealthBelowPercentage = 50
  RepairMinReadyTime = 66
  RepairMaxReadyTime = 66
  RepairMinWeldTime = 100
  RepairMaxWeldTime = 100
 End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(text, "slaved-owner.ini").unwrap();
    let drone = GameLogic::build_template_from_object_definition(
        "OwnedSlaveRepair",
        parser.get_definition("OwnedSlaveRepair").unwrap(),
        None,
    );
    let data = drone.slaved_update_metadata.as_ref().unwrap();
    assert_eq!((data.min_ready_frames, data.min_weld_frames), (2, 3));
    game.templates.insert(drone.name.clone(), drone);
    let master = game
        .create_object_for_player("OwnedSlaveMaster", 0, Vec3::ZERO)
        .unwrap();
    let drone = game
        .create_object_for_player("OwnedSlaveRepair", 0, Vec3::ZERO)
        .unwrap();
    assert!(
        game.host_object(drone)
            .unwrap()
            .selected_locomotor_definition
            .is_some(),
        "factory must bind the actual selected locomotor definition"
    );
    assert!(game.enslave_owned_drone(drone, master));
    (game, master, drone)
}
#[test]
fn receiving_repair_runs_unpack_extend_then_real_healing_once_without_peer_influence() {
    let (mut game, master, drone) = world();
    let (mut peer, peer_master, peer_drone) = world();
    game.frame = 100;
    peer.frame = 700;
    game.host_object_mut(master)
        .unwrap()
        .internal_change_health(-60.0);
    game.update_owned_slaved_object(drone);
    assert_eq!(
        game.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .repair_state,
        Repair::Unpacking
    );
    assert_eq!(game.host_object(master).unwrap().health.current, 40.0);
    for _ in 0..15 {
        game.update_owned_slaved_object(drone);
    }
    assert_eq!(
        game.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .repair_state,
        Repair::Ready
    );
    for _ in 0..2 {
        game.update_owned_slaved_object(drone);
    }
    assert_eq!(
        game.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .repair_state,
        Repair::Extending
    );
    for _ in 0..5 {
        game.update_owned_slaved_object(drone);
    }
    assert_eq!(
        game.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .repair_state,
        Repair::Welding
    );
    assert_eq!(game.host_object(master).unwrap().health.current, 41.0);
    assert_eq!(game.battle_drone_residual_repairs, 1);
    peer.update_owned_slaved_object(peer_drone);
    assert_eq!(peer.host_object(peer_master).unwrap().health.current, 100.0);
    assert_eq!(game.host_object(master).unwrap().health.current, 41.0);
}
#[test]
fn missing_master_stops_slave_effects_before_unmanned_idle() {
    let (mut game, master, drone) = world();
    game.host_object_mut(drone)
        .unwrap()
        .set_status_disabled_held(true);
    game.objects.remove(&master);
    game.update_owned_slaved_object(drone);
    let owner = game.host_object(drone).unwrap();
    assert!(!owner.status.unselectable);
    assert!(!owner.status.disabled_held);
    assert!(owner.status.disabled_unmanned);
    assert_eq!(owner.ai_state, crate::game_logic::AIState::Idle);
    assert_eq!(owner.slaved_update.as_ref().unwrap().master, None);
    assert_eq!(
        owner.slaved_update.as_ref().unwrap().guard_offset,
        Vec3::ZERO
    );
}

#[test]
fn normal_encoded_save_preserves_exact_repair_wait_and_resumed_body_healing() {
    let (mut game, master, drone) = world();
    game.host_object_mut(master)
        .unwrap()
        .internal_change_health(-60.0);
    game.update_owned_slaved_object(drone);
    for _ in 0..7 {
        game.update_owned_slaved_object(drone);
    }
    let saved = game
        .host_object(drone)
        .unwrap()
        .slaved_update
        .clone()
        .unwrap();
    let directory = tempfile::TempDir::new().unwrap();
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let info = crate::save_load::SaveGameInfo {
        filename: "slave_owner".into(),
        display_name: "Slaved continuation".into(),
        description: "Actual module wait".into(),
        map_name: "SlaveOwnerMap".into(),
        pristine_map_name: None,
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    };
    manager.save_game("slave_owner", &game, &info).unwrap();
    let (decoded, _) = manager.load_game_snapshot("slave_owner").unwrap();
    let mut restored = GameLogic::new();
    restored.templates = game.templates.clone();
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .unwrap();
    let actual = restored
        .host_object(drone)
        .unwrap()
        .slaved_update
        .as_ref()
        .unwrap();
    assert_eq!(actual.master, saved.master);
    assert_eq!(actual.guard_offset, saved.guard_offset);
    assert_eq!(actual.frames_to_wait, saved.frames_to_wait);
    assert_eq!(actual.repair_state, saved.repair_state);
    assert_eq!(actual.repairing, saved.repairing);
    for _ in 0..15 {
        game.update_owned_slaved_object(drone);
        restored.update_owned_slaved_object(drone);
    }
    assert_eq!(
        restored.host_object(master).unwrap().health.current,
        game.host_object(master).unwrap().health.current
    );
    assert!(restored.host_object(master).unwrap().health.current > 40.0);
    // CPP xfer stores m_slaver verbatim; a removed master is resolved by the
    // next actual update, not rejected or repaired by loading.
    game.objects.remove(&master);
    manager.save_game("slave_missing", &game, &info).unwrap();
    let (missing, _) = manager.load_game_snapshot("slave_missing").unwrap();
    let mut lost = GameLogic::new();
    lost.templates = game.templates.clone();
    manager.restore_game_snapshot(&missing, &mut lost).unwrap();
    assert_eq!(
        lost.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .master,
        Some(master)
    );
    assert!(!lost.host_object(drone).unwrap().status.disabled_unmanned);
    lost.update_owned_slaved_object(drone);
    assert_eq!(
        lost.host_object(drone)
            .unwrap()
            .slaved_update
            .as_ref()
            .unwrap()
            .master,
        None
    );
    assert!(lost.host_object(drone).unwrap().status.disabled_unmanned);
}

#[test]
fn captured_master_transfers_actual_slave_owner_before_repair_observation() {
    let (mut game, master, drone) = world();
    let (peer, _, peer_drone) = world();
    game.add_player(Player::new(1, Team::China, "captor", false));
    game.objects.get_mut(&master).unwrap().set_team_and_owner(
        Team::China,
        Some(1),
        &mut game.ownership_events,
    );
    game.update_owned_slaved_object(drone);
    let slave = game.host_object(drone).unwrap();
    assert_eq!(slave.team, Team::China);
    assert_eq!(slave.owner_player_id, Some(1));
    assert_eq!(slave.slaved_update.as_ref().unwrap().master, Some(master));
    assert_eq!(
        peer.host_object(peer_drone).unwrap().owner_player_id,
        Some(0)
    );
}

#[test]
fn missing_selected_locomotor_does_not_run_or_mutate_slave_callback() {
    let (mut game, master, drone) = world();
    game.host_object_mut(master)
        .unwrap()
        .internal_change_health(-60.0);
    game.host_object_mut(drone)
        .unwrap()
        .selected_locomotor_definition = None;
    let before = game
        .host_object(drone)
        .unwrap()
        .slaved_update
        .clone()
        .unwrap();
    game.update_owned_slaved_object(drone);
    let after = game
        .host_object(drone)
        .unwrap()
        .slaved_update
        .as_ref()
        .unwrap();
    assert_eq!(after.frames_to_wait, before.frames_to_wait);
    assert_eq!(after.repair_state, before.repair_state);
    assert_eq!(game.host_object(master).unwrap().health.current, 40.0);
    assert_eq!(game.battle_drone_residual_repairs, 0);
}
