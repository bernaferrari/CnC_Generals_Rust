//! Candidate-only command/path diagnostics supporting the frozen turret witness.
//! C++ AIUpdateInterface::privateAttackObject installs the state and goal;
//! turning-turret AIM itself does not request locomotor movement.
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    AIState, AttackSubState, GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate,
    Weapon,
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
        "ai":format!("{:?}",object.ai_state),"attack_state":format!("{:?}",object.attack_substate)})
}

fn command_case(distance: f32) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::GLA)] {
        let mut player = Player::new(id, team, "Turret path boundary", id == 0);
        player.alliance_team = id as i32;
        logic.add_player(player);
    }
    let mut tank = ThingTemplate::new("AmericaTankCrusader");
    tank.set_health(1000.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon(Weapon {
            damage: 25.0,
            range: 300.0,
            reload_time: 1.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        });
    logic.templates.insert(tank.name.clone(), tank);
    for (name, kind) in [
        ("PathVictim", KindOf::Vehicle),
        ("PathAnchor", KindOf::Structure),
    ] {
        let mut template = ThingTemplate::new(name);
        template
            .set_health(1000.0)
            .add_kind_of(kind)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_none();
        logic.templates.insert(name.into(), template);
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
    assert_eq!(source.ai_state, AIState::Attacking);
    assert_eq!(source.attack_substate, AttackSubState::AimAtTarget);
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
}
