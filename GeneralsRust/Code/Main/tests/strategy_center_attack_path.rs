//! Candidate-only command/path diagnostics supporting the frozen turret witness.
//! C++ AIUpdateInterface::privateAttackObject installs the state and goal;
//! turning-turret AIM itself does not request locomotor movement.
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    AIState, AttackSubState, GameLogic, Object, ObjectId, Player, Team,
};
use glam::Vec3;
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};

fn path_state(logic: &GameLogic, id: ObjectId) -> Value {
    let object = logic.host_object(id).unwrap();
    json!({"frame":logic.get_frame(),"position":object.get_position().to_array(),
        "body_yaw":object.get_orientation(),"moving":object.status.moving,
        "path":object.movement.path.iter().map(|p|p.to_array()).collect::<Vec<_>>(),
        "path_index":object.movement.current_path_index,
        "move_target":object.movement.target_position.map(|p|p.to_array()),
        "requested_destination":object.requested_destination.map(|p|p.to_array()),
        "path_goal":object.path_goal_position.map(|p|p.to_array()),
        "is_attack_path":object.is_attack_path,"waiting_for_path":object.waiting_for_path,
        "locomotor_goal":format!("{:?}",object.locomotor_goal_type),
        "ai":format!("{:?}",object.ai_state()),"attack_state":format!("{:?}",object.attack_substate())})
}

fn command_case(distance: f32) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors
        .apply_source("Locomotor TurretPathGround\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 120\n Appearance = FOUR_WHEELS\nEnd\n")
        .unwrap();
    logic.admit_locomotor_definitions(locomotors).unwrap();
    let objects = r#"
Object AmericaTankCrusader
  KindOf = VEHICLE SELECTABLE
  Locomotor = SET_NORMAL TurretPathGround
  Body = ActiveBody Body
    MaxHealth = 1000
    InitialHealth = 1000
  End
  Behavior = AIUpdateInterface Brain
    Turret
      TurretTurnRate = 180
      ControlledWeaponSlots = PRIMARY
    End
  End
  WeaponSet
    Conditions = NONE
    Weapon = PRIMARY TurretPathWeapon
  End
End
Object PathVictim
  KindOf = VEHICLE
  Body = ActiveBody Body
    MaxHealth = 1000
    InitialHealth = 1000
  End
End
Object PathAnchor
  KindOf = STRUCTURE
  Body = ActiveBody Body
    MaxHealth = 1000
    InitialHealth = 1000
  End
End
"#;
    logic
        .admit_object_ini_sources([("TurretPath.ini".into(), objects.into())])
        .unwrap();
    let mut weapon = gamelogic::weapon::WeaponTemplate::new("TurretPathWeapon".into());
    weapon.primary_damage = 25.0;
    weapon.attack_range = 150.0;
    weapon.projectile_name = "NONE".into();
    logic
        .admit_receiving_weapon_snapshot(vec![("TurretPathWeapon".into(), weapon)])
        .unwrap();
    for (id, team) in [(0, Team::USA), (1, Team::GLA)] {
        let mut player = Player::new(id, team, "Turret path boundary", id == 0);
        player.alliance_team = id as i32;
        logic.add_player(player);
    }
    let tank = logic
        .create_object_for_player("AmericaTankCrusader", 0, Vec3::ZERO)
        .unwrap();
    let anchor = logic
        .create_object_for_player("PathAnchor", 1, Vec3::new(2000.0, 0.0, 0.0))
        .unwrap();
    for _ in 0..210 {
        let source = logic.host_object(tank).unwrap();
        if Object::weapon_ready(
            source.weapon.as_ref().unwrap(),
            logic.get_frame() as f32 / 30.0,
        ) {
            break;
        }
        logic.update();
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        assert_eq!(logic.host_object(anchor).unwrap().health.current, 1000.0);
    }
    let victim = logic
        .create_object_for_player("PathVictim", 1, Vec3::new(0.0, 0.0, distance))
        .unwrap();
    let source = logic.host_object(tank).unwrap();
    assert_eq!(
        source.cur_locomotor_name.as_deref(),
        Some("TurretPathGround")
    );
    assert_eq!(source.owner_player_id, Some(0));
    assert_eq!(source.selected_weapon_slot(), Some(0));
    assert!(source.is_weapon_slot_on_turret(0) && source.turret_turn_rate_rad > 0.0);
    assert_eq!(source.weapon.as_ref().unwrap().range, 150.0);
    assert!(Object::weapon_ready(
        source.weapon.as_ref().unwrap(),
        logic.get_frame() as f32 / 30.0
    ));
    assert!(source.movement.path.is_empty() && source.movement.target_position.is_none());
    assert!(source.requested_destination.is_none());
    assert!(!logic.want_to_squish_target(tank, victim));
    assert_eq!(
        logic.out_of_weapon_range_object(tank, victim),
        distance > 150.0
    );
    println!(
        "TURRET_PATH_BEFORE distance={distance} {}",
        path_state(&logic, tank)
    );
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: victim },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![tank],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    let source = logic.host_object(tank).unwrap();
    assert_eq!(source.ai_state(), AIState::Attacking);
    assert_eq!(source.attack_substate(), AttackSubState::AimAtTarget);
    assert_eq!(source.target, Some(victim));
    assert!(source.status.is_aiming_weapon);
    println!(
        "TURRET_PATH_AFTER distance={distance} {}",
        path_state(&logic, tank)
    );
    (logic, tank, victim)
}

#[test]
fn in_range_turning_turret_command_does_not_add_a_movement_path() {
    let (mut logic, tank, _) = command_case(100.0);
    let source = logic.host_object(tank).unwrap();
    assert!(source.movement.path.is_empty());
    assert!(source.movement.target_position.is_none());
    assert!(source.requested_destination.is_none());
    assert!(!source.status.moving && !source.waiting_for_path);
    for _ in 0..30 {
        logic.update();
        let source = logic.host_object(tank).unwrap();
        assert_eq!(source.get_position(), Vec3::ZERO);
        assert_eq!(source.get_orientation(), 0.0);
    }
    assert!(
        logic
            .host_object(tank)
            .unwrap()
            .weapon_discharge_marker()
            .sequence
            > 0
    );
}

#[test]
fn out_of_range_turning_turret_command_still_requests_and_follows_a_path() {
    let (mut logic, tank, victim) = command_case(300.0);
    let (mut peer, peer_tank, peer_victim) = command_case(300.0);
    assert_eq!((tank, victim), (peer_tank, peer_victim));
    let source = logic.host_object(tank).unwrap();
    assert_eq!(
        source.requested_destination,
        Some(Vec3::new(0.0, 0.0, 300.0))
    );
    assert!(!source.movement.path.is_empty() || source.waiting_for_path);
    for _ in 0..90 {
        logic.update();
    }
    let source = logic.host_object(tank).unwrap();
    let target = logic.host_object(victim).unwrap();
    println!("TURRET_PATH_PROGRESS {}", path_state(&logic, tank));
    assert!(source.get_position().distance(target.get_position()) < 300.0);
    assert!(source.get_position().length() > 1.0);
    assert_eq!(source.owner_player_id, Some(0));
    assert_eq!(source.target, Some(victim));
    let progressed = source.get_position();
    assert_eq!(
        peer.host_object(peer_tank).unwrap().get_position(),
        Vec3::ZERO
    );
    for _ in 0..30 {
        peer.update();
    }
    assert!(peer.host_object(peer_tank).unwrap().get_position().length() > 1.0);
    assert_eq!(logic.host_object(tank).unwrap().get_position(), progressed);
}
