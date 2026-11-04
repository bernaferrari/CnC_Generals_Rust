//! Actual receiving SlavedUpdate callback order (CPP SlavedUpdate.cpp:91–261,410–674).
use crate::game_logic::host_slave_drones::BattleDroneRepairState as Repair;
use crate::game_logic::{GameLogic, Player, Team};
use glam::Vec3;
fn world() -> (
    GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    let mut game = receiving_world();
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
    game.host_object_mut(drone).unwrap().producer_id = Some(master);
    let master_birth =
        std::sync::Arc::downgrade(&game.host_object(master).unwrap().visual_draw_identity_token);
    assert!(game.enslave_owned_drone(drone, master, &master_birth));
    (game, master, drone)
}

#[test]
fn on_enslave_rejects_replaced_master_birth_but_accepts_queued_dead_original() {
    let (mut replaced, old_master, drone) = world();
    let expected = std::sync::Arc::downgrade(
        &replaced
            .host_object(old_master)
            .unwrap()
            .visual_draw_identity_token,
    );
    let mut replacement = replaced.objects.remove(&old_master).unwrap();
    replacement.visual_draw_identity_token = std::sync::Arc::new(());
    replaced.objects.insert(old_master, replacement);
    assert!(
        !replaced.enslave_owned_drone(drone, old_master, &expected),
        "same numeric ID with a new birth must not become the slave master"
    );

    let (mut queued_dead, master, drone) = world();
    let birth = std::sync::Arc::downgrade(
        &queued_dead
            .host_object(master)
            .unwrap()
            .visual_draw_identity_token,
    );
    queued_dead
        .host_object_mut(master)
        .unwrap()
        .status
        .effectively_dead = true;
    assert!(
        queued_dead.enslave_owned_drone(drone, master, &birth),
        "C++ retains the original Object pointer during queued deletion"
    );
}

/// One paired source owns both typed module data and the host projection.
/// Fresh receiving sessions use these same definitions before snapshot admission.
fn receiving_world() -> GameLogic {
    let mut game = GameLogic::new();
    // Actual authored locomotor + SlavedUpdate factory path; a mobile KindOf
    // alone is not CPP getCurLocomotor admission (SlavedUpdate.cpp:109–119).
    let text = r#"
Object OwnedSlaveMaster
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody MasterBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object SlaveVictim
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody VictimBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object OwnedSlaveRepair
 Body = ActiveBody DroneBody
  MaxHealth = 100
  InitialHealth = 100
 End
 KindOf = VEHICLE SELECTABLE
 Locomotor = SET_NORMAL ComancheLocomotor
 Behavior = AIUpdateInterface OwnedSlaveAI
  AutoAcquireEnemiesWhenIdle = Yes
 End
 Behavior = SlavedUpdate OwnedSlaveUpdate
  GuardMaxRange = 40
  AttackRange = 75
  ScoutRange = 75
  RepairRatePerSecond = 30
  RepairWhenBelowHealth% = 50
  RepairMinReadyTime = 66
  RepairMaxReadyTime = 66
  RepairMinWeldTime = 100
  RepairMaxWeldTime = 100
 End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(text, "slaved-owner.ini").unwrap();
    assert!(parser.get_definition("OwnedSlaveRepair").is_some());
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "slaved-owner.ini".into(),
            text.into(),
        )])
        .unwrap(),
    )
    .unwrap();
    let data = game.templates["OwnedSlaveRepair"]
        .slaved_update_metadata
        .as_ref()
        .unwrap();
    assert_eq!((data.min_ready_frames, data.min_weld_frames), (2, 3));
    assert_eq!(data.repair_priority_percent, 50);
    game.add_player(Player::new(0, Team::USA, "owner", true));
    game
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
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
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
        game.update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
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
        game.update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
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
        game.update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
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
    peer.update_owned_slaved_object(peer_drone)
        .expect("valid slaved fixture");
    assert_eq!(peer.host_object(peer_master).unwrap().health.current, 100.0);
    assert_eq!(game.host_object(master).unwrap().health.current, 41.0);
}
#[test]
fn missing_master_stops_slave_effects_before_unmanned_idle() {
    let (mut game, master, drone) = world();
    game.host_object_mut(drone)
        .unwrap()
        .set_status_disabled_held(true);
    // Remove the physical member as well; keep the slave's saved secondary
    // reference untouched to exercise missing-master resolution on update.
    game.take_object_for_physical_retirement(master, &mut Default::default())
        .unwrap()
        .expect("installed master at physical removal");
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
    let owner = game.host_object(drone).unwrap();
    assert!(!owner.status.unselectable);
    assert!(!owner.status.disabled_held);
    assert!(owner.status.disabled_unmanned);
    assert_eq!(owner.ai_state(), crate::game_logic::AIState::Idle);
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
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
    for _ in 0..7 {
        game.update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
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
    let crate::save_load::DecodedGameSave {
        snapshot: decoded, ..
    } = manager.load_game_snapshot("slave_owner").unwrap();
    // Admit the same paired content and player defaults before the saved roster.
    let mut restored = receiving_world();
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
        game.update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
        restored
            .update_owned_slaved_object(drone)
            .expect("valid slaved fixture");
    }
    assert_eq!(
        restored.host_object(master).unwrap().health.current,
        game.host_object(master).unwrap().health.current
    );
    assert!(restored.host_object(master).unwrap().health.current > 40.0);
    // CPP xfer stores m_slaver verbatim; a removed master is resolved by the
    // next actual update, not rejected or repaired by loading.
    // Remove the physical member as well; keep the slave's saved secondary
    // reference untouched to exercise missing-master resolution on update.
    game.take_object_for_physical_retirement(master, &mut Default::default())
        .unwrap()
        .expect("installed master at physical removal");
    manager.save_game("slave_missing", &game, &info).unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: missing, ..
    } = manager.load_game_snapshot("slave_missing").unwrap();
    let mut lost = receiving_world();
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
    lost.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
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
    assert!(game.transfer_object_to_player(master, 1));
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
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
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
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

fn move_goal(game: &GameLogic, drone: crate::game_logic::ObjectId) -> [f32; 3] {
    let owner = game.host_object(drone).unwrap();
    let goal = owner
        .movement
        .path
        .last()
        .copied()
        .expect("actual Slaved move must install a receiving path");
    assert_eq!(owner.requested_destination, Some(goal));
    assert_eq!(owner.path_goal_position, Some(goal));
    assert!(!owner.waiting_for_path);
    assert!(owner.status.moving);
    [goal.x, goal.y, goal.z]
}

#[test]
fn slaved_update_recalls_past_two_times_guard_max() {
    let (mut game, master, drone) = world();
    game.set_fixture_object_position(master, Vec3::new(100.0, 0.0, 0.0));
    game.host_object_mut(drone)
        .unwrap()
        .slaved_update
        .as_mut()
        .unwrap()
        .guard_offset = Vec3::ZERO;
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
    assert_eq!(move_goal(&game, drone), [100.0, 0.0, 0.0]);
    assert_eq!(
        game.host_object(drone).unwrap().ai_state(),
        crate::game_logic::AIState::Moving
    );
}
#[test]
fn slaved_update_scouts_ahead_of_actual_master_path() {
    let (mut game, master, drone) = world();
    game.host_object_mut(master).unwrap().movement.path =
        vec![Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0)];
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
    assert_eq!(move_goal(&game, drone), [75.0, 0.0, 0.0]);
}
#[test]
fn slaved_update_moves_toward_master_victim_not_nearest_enemy() {
    let (mut game, master, drone) = world();
    game.add_player(Player::new(1, Team::China, "enemy", false));
    // SlaveVictim is admitted alongside the master and drone above.
    let far = game
        .create_object_for_player("SlaveVictim", 1, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    let _near = game
        .create_object_for_player("SlaveVictim", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    game.host_object_mut(master).unwrap().target = Some(far);
    game.update_owned_slaved_object(drone)
        .expect("valid slaved fixture");
    assert_eq!(move_goal(&game, drone), [75.0, 0.0, 0.0]);
    assert_eq!(
        game.host_object(drone).unwrap().target,
        None,
        "Slaved attack logic issues movement, not forced weapon targeting"
    );
}

/// Shared authored fixture admission used by real containing slave controls.
/// Definition values are explicit fixture inputs; no name-based runtime logic.
pub(in crate::game_logic) fn install_slave_fixture(
    game: &mut GameLogic,
    kind: crate::game_logic::host_slave_drones::SlaveDroneKind,
) {
    use crate::game_logic::host_slave_drones::SlaveDroneKind;
    let name = kind.template_name();
    let primary = match kind {
        SlaveDroneKind::Battle => "BattleDroneMachineGun",
        SlaveDroneKind::Hellfire => "HellfireMissileWeapon",
        SlaveDroneKind::Scout => "",
    };
    // The actual authored Object needs its originating module namespace. Admit
    // every kind once: callers may request Scout then Battle before spawning.
    let text = [
        SlaveDroneKind::Scout,
        SlaveDroneKind::Battle,
        SlaveDroneKind::Hellfire,
    ]
    .into_iter()
    .map(|kind| {
        let (rate, vision) = match kind {
            SlaveDroneKind::Battle => (10, 150),
            SlaveDroneKind::Hellfire => (0, 100),
            SlaveDroneKind::Scout => (0, 150),
        };
        // Retail Object WeaponSet (hq-2qedt.130): create arms the authored
        // PRIMARY, not a template-name peel.
        let weapon_set = match kind {
            SlaveDroneKind::Battle => {
                "\n WeaponSet\n  Conditions = None\n  Weapon = PRIMARY BattleDroneMachineGun\n End"
            }
            SlaveDroneKind::Hellfire | SlaveDroneKind::Scout => "",
        };
        let name = kind.template_name();
        let upgrade = kind.upgrade_name();
        // Retail FactionUnit.ini authors the drone guns as a WeaponSet row.
        let weapon_set = match kind {
            SlaveDroneKind::Battle => {
                " WeaponSet\n  Conditions = None\n  Weapon = PRIMARY BattleDroneMachineGun\n End\n"
            }
            SlaveDroneKind::Hellfire => {
                " WeaponSet\n  Conditions = None\n  Weapon = PRIMARY HellfireMissileWeapon\n End\n"
            }
            SlaveDroneKind::Scout => "",
        };
        format!(
            r#"
Object {name}
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody FixtureSlaveBody
  MaxHealth = 100
  InitialHealth = 100
 End
