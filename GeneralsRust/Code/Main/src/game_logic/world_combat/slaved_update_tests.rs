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
  AttackRange = 75
  ScoutRange = 75
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
    // Remove the physical member as well; keep the slave's saved secondary
    // reference untouched to exercise missing-master resolution on update.
    let team_name = game.host_object(master).unwrap().team_instance_name.clone();
    game.unlink_leftover_team_host_member(master, &team_name);
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
    let crate::save_load::DecodedGameSave {
        snapshot: decoded, ..
    } = manager.load_game_snapshot("slave_owner").unwrap();
    let mut restored = GameLogic::new();
    // Load the same receiving player/default-team definitions before applying
    // the saved roster, just as map admission does in the production path.
    restored.add_player(Player::new(0, Team::USA, "owner", true));
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
    // Remove the physical member as well; keep the slave's saved secondary
    // reference untouched to exercise missing-master resolution on update.
    let team_name = game.host_object(master).unwrap().team_instance_name.clone();
    game.unlink_leftover_team_host_member(master, &team_name);
    game.objects.remove(&master);
    manager.save_game("slave_missing", &game, &info).unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: missing, ..
    } = manager.load_game_snapshot("slave_missing").unwrap();
    let mut lost = GameLogic::new();
    lost.add_player(Player::new(0, Team::USA, "owner", true));
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
    assert!(game.transfer_object_to_player(master, 1));
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
    game.host_object_mut(master)
        .unwrap()
        .set_position(Vec3::new(100.0, 0.0, 0.0));
    game.host_object_mut(drone)
        .unwrap()
        .slaved_update
        .as_mut()
        .unwrap()
        .guard_offset = Vec3::ZERO;
    game.update_owned_slaved_object(drone);
    assert_eq!(move_goal(&game, drone), [100.0, 0.0, 0.0]);
    assert_eq!(
        game.host_object(drone).unwrap().ai_state,
        crate::game_logic::AIState::Moving
    );
}
#[test]
fn slaved_update_scouts_ahead_of_actual_master_path() {
    let (mut game, master, drone) = world();
    game.host_object_mut(master).unwrap().movement.path =
        vec![Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0)];
    game.update_owned_slaved_object(drone);
    assert_eq!(move_goal(&game, drone), [75.0, 0.0, 0.0]);
}
#[test]
fn slaved_update_moves_toward_master_victim_not_nearest_enemy() {
    let (mut game, master, drone) = world();
    game.add_player(Player::new(1, Team::China, "enemy", false));
    let mut target = ThingTemplate::new("SlaveVictim");
    target
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    game.templates.insert(target.name.clone(), target);
    let far = game
        .create_object_for_player("SlaveVictim", 1, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    let _near = game
        .create_object_for_player("SlaveVictim", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    game.host_object_mut(master).unwrap().target = Some(far);
    game.update_owned_slaved_object(drone);
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
    let (rate, primary, vision) = match kind {
        SlaveDroneKind::Battle => (10, "BattleDroneMachineGun", 150),
        SlaveDroneKind::Hellfire => (0, "HellfireMissileWeapon", 100),
        SlaveDroneKind::Scout => (0, "", 150),
    };
    let name = kind.template_name();
    let upgrade = kind.upgrade_name();
    let text = format!(
        r#"
Object {name}
 KindOf = VEHICLE SELECTABLE ATTACKABLE
 VisionRange = {vision}
 Locomotor = SET_NORMAL ComancheLocomotor
 Behavior = AIUpdateInterface FixtureSlaveAI
  AutoAcquireEnemiesWhenIdle = Yes
 End
 Behavior = SlavedUpdate FixtureSlaved
  GuardMaxRange = 35
  GuardWanderRange = 35
  AttackRange = 75
  ScoutRange = 75
  RepairRatePerSecond = {rate}
  RepairWhenHealthBelowPercentage = 60
  RepairMinReadyTime = 300
  RepairMaxReadyTime = 750
  RepairMinWeldTime = 1000
  RepairMaxWeldTime = 1500
  RepairMinAltitude = 5
  RepairMaxAltitude = 10
  StayOnSameLayerAsMaster = Yes
 End
 ; Retail drones free the producer upgrade through their authored Die module.
 Behavior = UpgradeDie FixtureUpgradeDie
  DeathTypes = ALL
  UpgradeToRemove = {upgrade}
 End
End
"#
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&text, "containing-slave-owner.ini")
        .unwrap();
    let mut template = GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    );
    if !primary.is_empty() {
        template.set_primary_weapon_name(primary);
    }
    // Weapon.ini: HumveeGun129375, Hellfire129470, BattleDrone133256.
    // These are explicit receiving inputs, never an ambient bootstrap snapshot.
    let mut weapons: std::collections::HashMap<String, gamelogic::weapon::WeaponTemplate> = game
        .weapon_definition_base
        .as_ref()
        .map(|base| {
            base.iter()
                .map(|(name, rule)| (name.clone(), (**rule).clone()))
                .collect()
        })
        .unwrap_or_default();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut weapons,
        r#"
Weapon HumveeGun
 PrimaryDamage = 10
 PrimaryDamageRadius = 0
 AttackRange = 150
 DamageType = COMANCHE_VULCAN
 DeathType = NORMAL
 WeaponSpeed = 600
 ProjectileObject = NONE
 FireFX = WeaponFX_TechnicalGunFire
 VeterancyFireFX = HEROIC WeaponFX_GenericMachineGunFireWithRedTracers
 FireSound = HumveeWeapon
 RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
 DelayBetweenShots = 200
 ClipSize = 0
 ClipReloadTime = 0
End
Weapon BattleDroneMachineGun
 PrimaryDamage = 1
 PrimaryDamageRadius = 0
 AttackRange = 110
 DamageType = SMALL_ARMS
 DeathType = NORMAL
 WeaponSpeed = 999999
 ProjectileObject = NONE
 FireFX = WeaponFX_TechnicalGunFire
 FireSound = BattleDroneWeapon
 RadiusDamageAffects = ENEMIES NEUTRALS
 DelayBetweenShots = 100
 ClipSize = 0
 ClipReloadTime = 0
 WeaponBonus = PLAYER_UPGRADE DAMAGE 125%
End
Weapon HellfireMissileWeapon
 PrimaryDamage = 40
 PrimaryDamageRadius = 5
 ScatterRadiusVsInfantry = 10
 AttackRange = 150
 DamageType = EXPLOSION
 DeathType = EXPLODED
 WeaponSpeed = 600
 ProjectileObject = MissileDefenderMissile
 ProjectileExhaust = MissileDefenderMissileExhaust
 ProjectileDetonationFX = WeaponFX_RocketBuggyMissileDetonation
 FireSound = MissileDefenderWeapon
 RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
 DelayBetweenShots = 1000
 ClipSize = 1
 ClipReloadTime = 2000
 AutoReloadsClip = Yes
 AntiGround = Yes
 AntiAirborneVehicle = No
 AntiAirborneInfantry = No
 AcceptableAimDelta = 180
 ProjectileCollidesWith = STRUCTURES
End
"#,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    parser
        .parse_ini_content(
            r#"
Object MissileDefenderMissile
 KindOf = PROJECTILE SMALL_MISSILE
 Behavior = MissileAIUpdate FixtureMissile
  TryToFollowTarget = Yes
  FuelLifetime = 3000
  InitialVelocity = 150
  IgnitionDelay = 0
  DistanceToTravelBeforeTurning = 3
 End
End
"#,
            "receiving-slave-projectile.ini",
        )
        .unwrap();
    let definitions: Vec<_> = parser
        .get_all_definitions()
        .iter()
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect();
    let weapons = weapons
        .into_values()
        .map(|rule| (rule.name.clone(), rule))
        .collect();
    game.admit_weapon_content_snapshot(weapons, &definitions)
        .unwrap();
    for name in ["AmericaVehicleHumvee", "Humvee"] {
        if let Some(master) = game.templates.get_mut(name) {
            master.set_primary_weapon_name("HumveeGun");
        }
    }

    assert!(template.slaved_update_metadata.is_some());
    game.templates.insert(name.into(), template);
}

fn named_master_team(game: &mut GameLogic, master: crate::game_logic::ObjectId) -> u32 {
    let identity = {
        let mut factory = game.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "SlaveMasterTeam".into(),
                "owner".into(),
                false,
                None,
                Some(0),
            )
            .unwrap();
        let team = factory.create_inactive_team("SlaveMasterTeam").unwrap();
        let identity = team.read().unwrap().get_id();
        identity
    };
    assert!(game.set_host_object_team(master, identity));
    identity
}

#[test]
fn same_player_hostile_team_slave_defects_to_exact_master_instance_before_repair() {
    let (mut game, master, drone) = world();
    let (peer, _, peer_drone) = world();
    let old = game.host_object(drone).unwrap().team_instance_id.unwrap();
    let destination = named_master_team(&mut game, master);
    // A newer same-name sibling cannot become the destination by name lookup.
    game.team_factory
        .lock()
        .unwrap()
        .create_inactive_team("SlaveMasterTeam")
        .unwrap();
    game.team_factory
        .lock()
        .unwrap()
        .find_team_by_id(destination)
        .unwrap()
        .write()
        .unwrap()
        .set_override_team_relationship(old, gamelogic::common::Relationship::Enemies);
    game.host_object_mut(drone).unwrap().set_ai_attitude_i8(2);
    game.update_owned_slaved_object(drone);
    let slave = game.host_object(drone).unwrap();
    assert_eq!(slave.team_instance_id, Some(destination));
    assert_eq!(slave.team_instance_name, "SlaveMasterTeam");
    assert_eq!(slave.owner_player_id, Some(0));
    assert_eq!(slave.ai_attitude, 0);
    assert!(
        !slave.is_undetected_defector(),
        "CPP defect(masterTeam, 0) grants no cover"
    );
    assert_eq!(slave.slaved_update.as_ref().unwrap().master, Some(master));
    let factory = game.team_factory.lock().unwrap();
    assert!(
        !factory
            .find_team_by_id(old)
            .unwrap()
            .read()
            .unwrap()
            .has_member(drone.0)
    );
    assert_eq!(
        factory
            .find_team_by_id(destination)
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        &[drone.0, master.0]
    );
    assert_eq!(
        peer.host_object(peer_drone).unwrap().team_instance_id,
        Some(old)
    );
}

#[test]
fn slave_defect_uses_original_contained_sold_and_construction_status_gates() {
    for gate in 0..3 {
        let (mut game, master, drone) = world();
        let old = game.host_object(drone).unwrap().team_instance_id.unwrap();
        let destination = named_master_team(&mut game, master);
        game.team_factory
            .lock()
            .unwrap()
            .find_team_by_id(destination)
            .unwrap()
            .write()
            .unwrap()
            .set_override_team_relationship(old, gamelogic::common::Relationship::Enemies);
        let object = game.host_object_mut(drone).unwrap();
        match gate {
            0 => object.set_contained_by(Some(master)),
            1 => object.status.sold = true,
            _ => object.status.under_construction = true,
        }
        game.update_owned_slaved_object(drone);
        assert_eq!(game.host_object(drone).unwrap().team_instance_id, Some(old));
        assert!(game.host_object(drone).unwrap().defection_helper.is_none());
    }
    let (mut game, master, drone) = world();
    let destination = named_master_team(&mut game, master);
    game.host_object_mut(drone).unwrap().construction_percent = 0.2;
    assert!(
        game.defect_host_object(drone, destination, 0),
        "CPP gates the status bit, not a progress epsilon"
    );
}
