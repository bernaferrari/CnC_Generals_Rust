//! CPP Object3336 / VeterancyCrateCollide IsPilot: selected effective template.
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world(locomotor: &str) -> (GameLogic, ObjectId, ObjectId) {
    world_with_kind(locomotor, "VEHICLE SELECTABLE")
}
fn world_with_kind(locomotor: &str, vehicle_kind: &str) -> (GameLogic, ObjectId, ObjectId) {
    world_with_training(locomotor, vehicle_kind, "Yes")
}
fn world_with_training(
    locomotor: &str,
    vehicle_kind: &str,
    trainable: &str,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut owner = GameLogic::new();
    owner.add_player(Player::new(1, Team::USA, "PilotOwner", false));
    let text = format!(
        r#"
Object OwnedPilot
 IsTrainable = Yes
 KindOf = INFANTRY SELECTABLE
 Behavior = AIUpdateInterface PilotAI
 End
 Behavior = VeterancyGainCreate PilotRank
  StartingLevel = VETERAN
 End
 Behavior = VeterancyCrateCollide PilotCollide
  IsPilot = Yes
  RequiredKindOf = VEHICLE
  ForbiddenKindOf = DOZER
  EffectRange = 0
  AddsOwnerVeterancy = Yes
 End
End
Object OwnedVehicle
 IsTrainable = {trainable}
 KindOf = {vehicle_kind}
 Locomotor = SET_NORMAL {locomotor}
 Behavior = AIUpdateInterface VehicleAI
 End
End
"#
    );
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&text, "pilot-owner.ini").unwrap();
    for name in ["OwnedPilot", "OwnedVehicle"] {
        let template = GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).unwrap(),
            None,
        );
        assert_eq!(template.authored_ai_update_interface(), Some(true));
        owner.templates.insert(name.into(), template);
    }
    let pilot = owner
        .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        owner.host_object(pilot).unwrap().experience.level,
        crate::game_logic::VeterancyLevel::Veteran,
        "actual companion VeterancyGainCreate must admit a positive pilot transfer"
    );
    let vehicle = owner
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let vehicle_object = owner.host_object_mut(vehicle).unwrap();
    assert!(
        vehicle_object.selected_locomotor_definition.is_some(),
        "real factory must bind selected definition"
    );
    vehicle_object.status.disabled_unmanned = true;
    (owner, pilot, vehicle)
}

#[test]
fn pilot_admission_and_acquisition_observe_selected_air_not_targeting_status() {
    let (mut ground, pilot, vehicle) = world("CrusaderLocomotor");
    let (mut air, peer_pilot, peer_vehicle) = world("ComancheLocomotor");
    assert_eq!((pilot, vehicle), (peer_pilot, peer_vehicle));
    ground
        .host_object_mut(vehicle)
        .unwrap()
        .status
        .airborne_target = true;
    air.host_object_mut(peer_vehicle)
        .unwrap()
        .status
        .airborne_target = false;
    assert!(
        !ground
            .host_object(vehicle)
            .unwrap()
            .selected_locomotor_is_airborne()
    );
    assert!(
        air.host_object(peer_vehicle)
            .unwrap()
            .selected_locomotor_is_airborne()
    );
    assert!(ground.can_execute_pilot_recrew(pilot, vehicle));
    assert!(!air.can_execute_pilot_recrew(peer_pilot, peer_vehicle));
    ground.frame = 30;
    air.frame = 30;
    air.try_pilot_find_vehicle_residual(peer_pilot);
    ground.try_pilot_find_vehicle_residual(pilot);
    assert_eq!(ground.host_object(pilot).unwrap().target, Some(vehicle));
    assert_eq!(air.host_object(peer_pilot).unwrap().target, None);
    assert_eq!(
        ground.host_object(vehicle).unwrap().owner_player_id,
        Some(1)
    );
}

#[test]
fn aircraft_with_actual_selected_ground_template_remains_recrewable() {
    let (owner, pilot, vehicle) =
        world_with_kind("CrusaderLocomotor", "VEHICLE AIRCRAFT SELECTABLE");
    assert!(
        owner
            .host_object(vehicle)
            .unwrap()
            .is_kind_of(crate::game_logic::KindOf::Aircraft)
    );
    assert!(
        !owner
            .host_object(vehicle)
            .unwrap()
            .selected_locomotor_is_airborne()
    );
    assert!(owner.can_execute_pilot_recrew(pilot, vehicle));
}

#[test]
fn selected_air_admission_reconstructs_through_normal_world_save() {
    use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
    let (owner, pilot, vehicle) = world("ComancheLocomotor");
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = owner.templates.clone();
    restored.add_player(Player::new(1, Team::USA, "PilotOwner", false));
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(
        restored
            .host_object(vehicle)
            .unwrap()
            .selected_locomotor_is_airborne()
    );
    assert!(!restored.can_execute_pilot_recrew(pilot, vehicle));
    restored.frame = 30;
    restored.try_pilot_find_vehicle_residual(pilot);
    assert_eq!(restored.host_object(pilot).unwrap().target, None);
}

#[test]
fn no_ai_or_no_selected_locomotor_is_not_air_even_with_aircraft_kind() {
    let (mut owner, _, vehicle) = world("ComancheLocomotor");
    let actor = owner.host_object_mut(vehicle).unwrap();
    actor
        .template_mut()
        .add_kind_of(crate::game_logic::KindOf::Aircraft);
    actor
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    assert!(!actor.selected_locomotor_is_airborne());
    actor
        .template_mut()
        .set_authored_ai_update_interface(Some(true));
    actor.cur_locomotor_name = None;
    actor.selected_locomotor_definition = None;
    assert!(!actor.selected_locomotor_is_airborne());
}

#[test]
fn parsed_nontrainable_ground_vehicle_rejects_pilot_entry() {
    let (mut owner, pilot, vehicle) =
        world_with_training("CrusaderLocomotor", "VEHICLE SELECTABLE", "No");
    let target = owner.host_object(vehicle).unwrap();
    assert!(!target.selected_locomotor_is_airborne());
    assert!(!target.is_trainable());
    assert!(!owner.can_execute_pilot_recrew(pilot, vehicle));
    owner.frame = 30;
    owner.try_pilot_find_vehicle_residual(pilot);
    assert_eq!(owner.host_object(pilot).unwrap().target, None);
}
