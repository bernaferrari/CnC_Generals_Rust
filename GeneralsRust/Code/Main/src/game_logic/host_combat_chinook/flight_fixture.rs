//! Authored object/locomotor fixture shared by owned Chinook flight regressions.

use super::HostChinookAIState;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, DropTarget, GameCommand, ModifierKeys};
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use glam::Vec3;

pub(super) fn authored_match() -> (GameLogic, ObjectId, ObjectId) {
    use game_engine::common::ini::ini_locomotor::load_locomotors_from_str;
    assert_eq!(
        load_locomotors_from_str(
            r#"
Locomotor Hq46k12ChinookLocomotor
  Surfaces = AIR
  Speed = 30
  SpeedDamaged = 30
  TurnRate = 180
  Acceleration = 100
  AccelerationDamaged = 100
  Braking = 100
  Lift = 300
  LiftDamaged = 300
  SpeedLimitZ = 30
  Appearance = HOVER
  PreferredHeight = 100
  PreferredHeightDamping = 1
  AllowAirborneMotiveForce = Yes
  ZAxisBehavior = SURFACE_RELATIVE_HEIGHT
End
"#
        )
        .expect("authored locomotor"),
        1
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object Hq46k12CombatChinook
  KindOf = PRELOAD SELECTABLE CAN_ATTACK VEHICLE AIRCRAFT TRANSPORT
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 350
  End
  Behavior = ChinookAIUpdate ModuleTag_AI
    MaxBoxes = 8
    NumRopes = 4
    MinDropHeight = 40
    RappelSpeed = 30
  End
  Behavior = TransportContain ModuleTag_Contain
    Slots = 2
    AllowInsideKindOf = INFANTRY
    PassengersAllowedToFire = Yes
  End
  Locomotor = SET_NORMAL Hq46k12ChinookLocomotor
End
Object Hq46k12Rappeller
  KindOf = PRELOAD SELECTABLE INFANTRY CAN_RAPPEL
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
End
"#,
            "hq46k12_chinook.ini",
        )
        .expect("authored object definitions");
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "FlightOwner", true));
    logic.add_player(Player::new(1, Team::GLA, "Opponent", false));
    for name in ["Hq46k12CombatChinook", "Hq46k12Rappeller"] {
        let template = GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).expect("authored definition"),
            None,
        );
        logic.templates.insert(name.to_owned(), template);
    }
    let transport = logic
        .create_object(
            "Hq46k12CombatChinook",
            Team::USA,
            Vec3::new(40.0, 100.0, 40.0),
        )
        .expect("authored transport");
    let passenger = logic
        .create_object("Hq46k12Rappeller", Team::USA, Vec3::new(35.0, 0.0, 40.0))
        .expect("authored passenger");
    let transport_object = logic.host_object_mut(transport).unwrap();
    assert!(
        transport_object.chinook_ai.is_some(),
        "actual spawn installed flight runtime"
    );
    assert!(
        (transport_object.movement.max_speed - 30.0).abs() < 0.001,
        "authored locomotor reached its actual owner"
    );
    assert_eq!(
        transport_object.loco_appearance,
        crate::game_logic::LocomotorAppearance::Hover
    );
    assert_eq!(
        transport_object.loco_behavior_z,
        crate::game_logic::LocomotorBehaviorZ::SurfaceRelativeHeight
    );
    assert!((transport_object.max_lift - 300.0 / 900.0).abs() < 0.0001);
    assert!((transport_object.max_lift_damaged - 300.0 / 900.0).abs() < 0.0001);
    assert!((transport_object.speed_limit_z - 30.0 / 30.0).abs() < 0.0001);
    assert!(transport_object.allow_motive_force_while_airborne);
    assert_ne!(
        transport_object.locomotor_surfaces & gamelogic::ai::pathfind_complete::SURFACE_AIR,
        0
    );
    assert!(transport_object.add_occupant(passenger));
    let passenger_object = logic.host_object_mut(passenger).unwrap();
    passenger_object.set_contained_by(Some(transport));
    passenger_object.set_ai_state(AIState::Docked);
    (logic, transport, passenger)
}

pub(super) fn combat_drop(logic: &mut GameLogic, transport: ObjectId) {
    let command = GameCommand {
        command_type: CommandType::CombatDrop {
            target: DropTarget::Location(Vec3::new(240.0, 0.0, 40.0)),
        },
        player_id: 0,
        command_id: 46_012,
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        selected_units: vec![transport],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(logic, 0)
            .execute_command(command)
            .unwrap(),
        CommandResult::Success
    );
    let object = logic.host_object(transport).unwrap();
    assert_eq!(
        object.chinook_ai.as_ref().unwrap().state,
        HostChinookAIState::MoveToCombatDrop
    );
    assert!(
        !object.movement.path.is_empty(),
        "real command installed the locomotor path"
    );
}
