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
    let mut owner = admitted_world(locomotor, vehicle_kind, trainable);
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

fn admitted_world(locomotor: &str, vehicle_kind: &str, trainable: &str) -> GameLogic {
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
 Behavior = PilotFindVehicleUpdate PilotScan
  ScanRate = 1000
  ScanRange = 300
  MinHealth = 0.5
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
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "pilot-owner.ini".to_string(),
        text,
    )])
    .unwrap();
    owner.admit_receiving_module_content(content).unwrap();
    for name in ["OwnedPilot", "OwnedVehicle"] {
        assert_eq!(
            owner.templates[name].authored_ai_update_interface(),
            Some(true)
        );
    }
    owner
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
    ground.host_object_mut(vehicle).unwrap().status.disabled_unmanned = false;
    air.host_object_mut(peer_vehicle).unwrap().status.disabled_unmanned = false;
    for world in [&mut ground, &mut air] {
        world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
        world.frame = 30;
    }
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
    let mut restored = admitted_world("ComancheLocomotor", "VEHICLE SELECTABLE", "Yes");
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

#[test]
fn pilot_enter_promotes_a_manned_vehicle_without_recapturing_it() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{AIState, VeterancyLevel};

    let mut world = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
    let pilot = world
        .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
        .unwrap();
    let vehicle = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::X)
        .unwrap();
    world
        .host_object_mut(vehicle)
        .unwrap()
        .set_experience_scalar(3.0);
    world.host_object_mut(pilot).unwrap().name = "AuthoredPilotName".into();
    world.initialize_receiving_named_cache_for_fixture();
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 1);
    let pilot_view = frame
        .objects
        .iter()
        .find(|object| object.id == pilot)
        .unwrap();
    let vehicle_view = frame
        .objects
        .iter()
        .find(|object| object.id == vehicle)
        .unwrap();
    assert_eq!(pilot_view.pilot_promotion_source_owner, Some(1));
    assert_eq!(vehicle_view.pilot_promotion_target_owner, Some(1));
    let team = world.host_object(vehicle).unwrap().team_instance_id;
    assert!(!world.host_object(vehicle).unwrap().is_unmanned());
    assert_eq!(
        world.host_object(vehicle).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert_eq!(
        world.host_object(pilot).unwrap().experience.level,
        VeterancyLevel::Veteran
    );

    world.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: vehicle },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![pilot],
        modifier_keys: ModifierKeys::default(),
    });
    world.process_commands();
    let approaching = world.host_object(pilot).unwrap();
    assert_eq!(approaching.ai_state(), AIState::Entering);
    assert_eq!(approaching.target, Some(vehicle));
    world
        .update_support_states(&[pilot, vehicle], 1.0 / 30.0)
        .unwrap();

    let target = world.host_object(vehicle).unwrap();
    assert_eq!(target.experience.level, VeterancyLevel::Veteran);
    assert_eq!(
        target.experience.current,
        target.get_template().veterancy_xp_thresholds[0]
    );
    assert_eq!(target.team_instance_id, team);
    assert_eq!(target.owner_player_id, Some(1));
    assert!(!target.is_private_captured());
    assert!(!target.is_unmanned());
    assert_eq!(world.host_named_unit_id("AuthoredPilotName"), Some(vehicle));
    assert!(
        world
            .host_object(pilot)
            .is_none_or(|object| object.status.destroyed)
    );
}

#[test]
fn pilot_crate_collision_requires_a_matching_goal_and_exact_controller() {
    use crate::game_logic::{AIState, VeterancyLevel};

    let mut world = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
    world.add_player(Player::new(2, Team::USA, "SameFactionPeer", false));
    let pilot = world
        .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
        .unwrap();
    let vehicle = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::X)
        .unwrap();
    let peer = world
        .create_object_for_player("OwnedVehicle", 2, Vec3::Z)
        .unwrap();
    let mut effects = crate::game_logic::ScriptPresentationEffects::default();

    world
        .dispatch_host_collide_modules_with_effects(pilot, vehicle, &mut effects)
        .unwrap();
    assert_eq!(
        world.host_object(vehicle).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert!(world.host_object(pilot).unwrap().is_alive());

    world
        .unit_command_stop_moving_order_target(pilot, Some(peer))
        .unwrap();
    world
        .unit_command_set_ai_state(pilot, AIState::Entering)
        .unwrap();
    world
        .dispatch_host_collide_modules_with_effects(pilot, peer, &mut effects)
        .unwrap();
    assert_eq!(
        world.host_object(peer).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert!(world.host_object(pilot).unwrap().is_alive());

    world
        .unit_command_stop_moving_order_target(pilot, Some(vehicle))
        .unwrap();
    world
        .unit_command_set_ai_state(pilot, AIState::Entering)
        .unwrap();
    world
        .dispatch_host_collide_modules_with_effects(pilot, vehicle, &mut effects)
        .unwrap();
    assert_eq!(
        world.host_object(vehicle).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    world
        .dispatch_host_collide_modules_with_effects(pilot, vehicle, &mut effects)
        .unwrap();
    assert_eq!(
        world.host_object(vehicle).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    assert!(
        world
            .host_object(pilot)
            .is_none_or(|object| object.status.destroyed)
    );
}

#[test]
fn ai_pilot_selects_closest_eligible_occupied_vehicle() {
    let mut world = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
    world.add_player(Player::new(2, Team::USA, "DifferentController", false));
    let pilot = world
        .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
        .unwrap();
    let enemy_owner = world
        .create_object_for_player("OwnedVehicle", 2, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let off_map = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let damaged = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let nearest = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let farther = world
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(world.host_object(enemy_owner).unwrap().team, Team::USA);
    world.host_object_mut(off_map).unwrap().status.off_map = true;
    let wounded = world.host_object_mut(damaged).unwrap();
    wounded.health.current = wounded.health.maximum * 0.25;
    assert!(!world.host_object(nearest).unwrap().is_unmanned());
    assert!(!world.host_object(farther).unwrap().is_unmanned());

    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.frame = 30;
    world.try_pilot_find_vehicle_residual(pilot);
    assert_eq!(world.host_object(pilot).unwrap().target, Some(nearest));
}

#[test]
fn pilot_scan_uses_birth_relative_wake_frame_across_save() {
    use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};

    let mut source = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
    let scan_frames = source.templates["OwnedPilot"]
        .pilot_find_vehicle
        .unwrap()
        .scan_frames;
    let birth_frame = 17;
    source.frame = birth_frame;
    let pilot = source
        .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
        .unwrap();
    let first_scan = birth_frame + 1;
    source.frame = first_scan;
    source.try_pilot_find_vehicle_residual(pilot);
    assert_eq!(source.host_object(pilot).unwrap().target, None);

    let vehicle = source
        .create_object_for_player("OwnedVehicle", 1, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    source
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    source.frame = scan_frames;
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.frame, source.frame);

    for world in [&mut source, &mut restored] {
        for early_frame in [scan_frames, scan_frames + 1, first_scan + scan_frames - 1] {
            world.frame = early_frame;
            world.try_pilot_find_vehicle_residual(pilot);
            assert_eq!(
                world.host_object(pilot).unwrap().target,
                None,
                "pilot scanned too early at frame {early_frame}"
            );
        }
        world.frame = first_scan + scan_frames;
        world.try_pilot_find_vehicle_residual(pilot);
        assert_eq!(world.host_object(pilot).unwrap().target, Some(vehicle));
    }
}

#[test]
fn disabled_pilot_retries_next_frame_and_human_pilot_stays_asleep() {
    for human in [false, true] {
        let mut world = admitted_world("CrusaderLocomotor", "VEHICLE SELECTABLE", "Yes");
        world.get_player_mut(1).unwrap().is_human = human;
        world.frame = 17;
        let pilot = world
            .create_object_for_player("OwnedPilot", 1, Vec3::ZERO)
            .unwrap();
        let vehicle = world
            .create_object_for_player("OwnedVehicle", 1, Vec3::X)
            .unwrap();
        world
            .update_owned_partition_with_effects(&mut Default::default())
            .unwrap();
        world
            .host_object_mut(pilot)
            .unwrap()
            .set_status_disabled_emp(!human);

        world.frame = 18;
        world.try_pilot_find_vehicle_residual(pilot);
        assert_eq!(world.host_object(pilot).unwrap().target, None);

        world.get_player_mut(1).unwrap().is_human = false;
        world
            .host_object_mut(pilot)
            .unwrap()
            .set_status_disabled_emp(false);
        world.frame = 19;
        world.try_pilot_find_vehicle_residual(pilot);
        assert_eq!(
            world.host_object(pilot).unwrap().target,
            (!human).then_some(vehicle)
        );

        if human {
            world.frame = 60;
            world.try_pilot_find_vehicle_residual(pilot);
            assert_eq!(world.host_object(pilot).unwrap().target, None);
        }
    }
}
