//! Installed containment onDie ordering, independent of physical destruction.
use super::*;
use crate::game_logic::object::Object;

fn parsed_world(
    container_module: &str,
    death_module: &str,
    percent: u32,
) -> (GameLogic, ObjectId, ObjectId) {
    parsed_world_with_contain_attributes(container_module, death_module, percent, "")
}

fn parsed_world_with_contain_attributes(
    container_module: &str,
    death_module: &str,
    percent: u32,
    contain_attributes: &str,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut owner = GameLogic::new();
    let ocl_sources = ["OwnedRiderDeathWitness", "OwnedContainerDeathWitness"].map(|name| {
        format!(
            "ObjectCreationList {name}\nCreateObject\nObjectNames = {name}\nCount = 1\nEnd\nEnd\n"
        )
    });
    owner.admit_ocl_application_sources(&ocl_sources).unwrap();
    // The existing native immutable module parser resolves OCL references here.
    // Register the same parsed products; runtime execution uses this world's
    // separately admitted definitions, not a guessed OCL-name object spawn.
    for name in ["OwnedRiderDeathWitness", "OwnedContainerDeathWitness"] {
        let list = owner.object_creation_list_definition(name).unwrap();
        gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(name, list);
    }
    // CaveContain inherits OpenContain's ContainMax; Slots belongs to
    // TransportContain and is not a valid CaveContain field.
    let capacity_field = if container_module == "CaveContain" {
        "ContainMax"
    } else {
        "Slots"
    };
    let input = format!(
        r#"
Object OwnedContainDeath
  KindOf = VEHICLE CAN_ATTACK
  Body = ActiveBody BodyTag
    MaxHealth = 200
    InitialHealth = 200
  End
  Behavior = {container_module} ContainTag
    {capacity_field} = 5
    DamagePercentToUnits = {percent}%
    AllowInsideKindOf = INFANTRY
    {contain_attributes}
  End
  {death_module}
End
Object OwnedContainRider
  KindOf = INFANTRY CAN_ATTACK
  TransportSlotCount = 1
  Locomotor = SET_NORMAL BasicHumanLocomotor
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface AiTag
  End
  Behavior = InstantDeathBehavior DeathTag
    OCL = OwnedRiderDeathWitness
  End
End
Object OwnedRiderDeathWitness
End
Object OwnedContainerDeathWitness
End
"#
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "OwnedContainDeath.ini".to_string(),
        input,
    )])
    .unwrap();
    owner.admit_receiving_module_content(content).unwrap();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner.add_player(Player::new(1, Team::USA, "ContainOwner", true));
    let container = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let rider = owner
        .create_object_for_player("OwnedContainRider", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(
        owner
            .host_object(rider)
            .unwrap()
            .cur_locomotor_name
            .is_some()
    );
    if owner
        .host_object(container)
        .unwrap()
        .is_cave_style_container()
    {
        let index = owner.host_object(container).unwrap().cave_index;
        assert!(
            owner
                .cave_system
                .record_enter(index, rider, container, Team::USA)
                .0
        );
    } else {
        assert!(
            owner
                .host_object_mut(container)
                .unwrap()
                .add_occupant(rider)
        );
    }
    owner.set_fixture_object_contained_by(rider, Some(container));
    (owner, container, rider)
}
fn kill(owner: &mut GameLogic, id: ObjectId) {
    assert!(
        owner
            .apply_owned_kill(
                id,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            )
            .unwrap()
            .destroyed()
    );
}

#[test]
fn authored_passenger_death_type_preserves_default_override_and_saved_continuation() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    for (attributes, expected) in [
        ("", HostDeathType::Burned),
        ("BurnedDeathToUnits = Yes", HostDeathType::Burned),
        ("BurnedDeathToUnits = No", HostDeathType::Normal),
    ] {
        let (mut owner, container, rider) =
            parsed_world_with_contain_attributes("TransportContain", "", 100, attributes);
        let builder = crate::save_load::SnapshotBuilder::new();
        let bytes =
            bincode_legacy::serialize(&builder.create_world_snapshot(&owner).unwrap()).unwrap();
        let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _, _) =
            parsed_world_with_contain_attributes("TransportContain", "", 100, attributes);
        builder
            .restore_from_snapshot(&saved, &mut restored)
            .unwrap();
        for receiving in [&mut owner, &mut restored] {
            kill(receiving, container);
            let passenger = receiving.host_object(rider).unwrap();
            assert_eq!(passenger.health.current, 0.0);
            assert_eq!(passenger.status.death_type, expected);
            assert!(passenger.status.on_die_started);
            assert!(
                receiving
                    .host_objects()
                    .registered_values()
                    .any(|object| object.template_name == "OwnedRiderDeathWitness")
            );
        }
    }
}

#[test]
fn older_json_containment_definition_keeps_original_burned_default() {
    let mut value = serde_json::to_value(ContainModuleMetadata::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("burned_death_to_units");
    let old: ContainModuleMetadata = serde_json::from_value(value).unwrap();
    assert!(old.burned_death_to_units);
}

#[test]
fn unfree_rider_kill_completes_receiving_callbacks_before_container_death_effect() {
    let module = "Behavior = InstantDeathBehavior DeathTag\nOCL = OwnedContainerDeathWitness\nEnd";
    let (mut owner, container, rider) = parsed_world("TransportContain", module, 0);
    let (peer, peer_container, peer_rider) = parsed_world("TransportContain", module, 0);
    assert_eq!((container, rider), (peer_container, peer_rider));
    owner.frame = 42;
    owner.host_object_mut(rider).unwrap().cur_locomotor_name = None;
    kill(&mut owner, container);
    let passenger = owner.host_object(rider).unwrap();
    assert_eq!(passenger.health.current, 0.0);
    assert_eq!(passenger.last_damage_timestamp, Some(42));
    assert!(passenger.status.on_die_started);
    assert_eq!(passenger.contained_by, None);
    let witness = |name: &str| {
        owner
            .host_objects()
            .registered_values()
            .find(|object| object.template_name == name)
            .unwrap()
            .id
    };
    assert!(witness("OwnedRiderDeathWitness").0 < witness("OwnedContainerDeathWitness").0);
    assert_eq!(peer.host_object(rider).unwrap().health.current, 100.0);
    assert!(!peer.host_object(container).unwrap().status.on_die_started);
}

#[test]
fn unfree_rider_indestructible_kill_does_not_force_death_dispatch() {
    let (mut owner, container, rider) = parsed_world("TransportContain", "", 0);
    let passenger = owner.host_object_mut(rider).unwrap();
    passenger.cur_locomotor_name = None;
    passenger.indestructible = true;
    kill(&mut owner, container);
    let passenger = owner.host_object(rider).unwrap();
    assert_eq!(passenger.health.current, 100.0);
    assert!(!passenger.status.on_die_started);
    assert_eq!(passenger.contained_by, None);
    assert!(
        !owner
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "OwnedRiderDeathWitness")
    );
}
#[test]
fn slow_death_observes_removed_damaged_rider_before_retained_owner_save_and_clone() {
    let module = "Behavior = SlowDeathBehavior DeathTag\nDestructionDelay = 5000\nEnd";
    let (mut owner, container, rider) = parsed_world("TransportContain", module, 50);
    let (mut peer, peer_container, peer_rider) = parsed_world("TransportContain", module, 50);
    assert_eq!((container, rider), (peer_container, peer_rider));
    owner.frame = 42;
    kill(&mut owner, container);
    let body = owner.host_object(container).unwrap();
    assert!(
        body.slow_death
            .as_ref()
            .is_some_and(|state| state.is_active())
    );
    assert!(body.status.on_die_started);
    assert!(body.contained_units().is_empty());
    let passenger = owner.host_object(rider).unwrap();
    assert_eq!(passenger.health.current, 50.0);
    assert_eq!(passenger.contained_by, None);
    assert_eq!(passenger.ai_state(), AIState::Moving);
    assert_eq!(peer.host_object(rider).unwrap().health.current, 100.0);
    assert_eq!(
        peer.host_object(rider).unwrap().contained_by,
        Some(container)
    );
    // Actual Object serialization and clone preserve the already completed
    // callback phase; this is a component continuation, not a full-game save.
    let bytes = serde_json::to_vec(body).unwrap();
    let restored: Object = serde_json::from_slice(&bytes).unwrap();
    peer.objects.insert(container, restored);
    peer.objects.insert(rider, passenger.clone());
    peer.mark_object_for_destruction(container, None);
    assert_eq!(peer.host_object(rider).unwrap().health.current, 50.0);
    owner.mark_object_for_destruction(container, None);
    owner.process_destroy_list();
    assert_eq!(owner.host_object(rider).unwrap().health.current, 50.0);
}
#[test]
fn instant_death_effect_follows_all_installed_rider_damage_callbacks() {
    let module = "Behavior = InstantDeathBehavior DeathTag\nOCL = OwnedContainerDeathWitness\nEnd";
    let (mut owner, container, first) = parsed_world("TransportContain", module, 100);
    let second = owner
        .create_object_for_player("OwnedContainRider", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(
        owner
            .host_object_mut(container)
            .unwrap()
            .add_occupant(second)
    );
    owner.set_fixture_object_contained_by(second, Some(container));
    kill(&mut owner, container);
    let container_witness = owner
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "OwnedContainerDeathWitness")
        .unwrap()
        .id;
    let rider_witnesses: Vec<_> = owner
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "OwnedRiderDeathWitness")
        .map(|object| object.id)
        .collect();
    assert_eq!(rider_witnesses.len(), 2);
    assert!(rider_witnesses.iter().all(|id| id.0 < container_witness.0));
    for id in [first, second] {
        assert!(!owner.host_object(id).unwrap().is_alive());
        assert_eq!(owner.host_object(id).unwrap().contained_by, None);
    }
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}
#[test]
fn authored_die_mux_keeps_roster_when_contain_death_filter_does_not_match() {
    let (mut owner, container, rider) = parsed_world("TransportContain", "", 100);
    owner
        .host_object_mut(container)
        .unwrap()
        .template_mut()
        .contain_module
        .death_rules = Some(ContainDeathRules::from_attributes(&[(
        "DeathTypes",
        "NONE +BURNED",
    )]));
    kill(&mut owner, container); // normal death deliberately does not match
    assert_eq!(owner.host_object(rider).unwrap().health.current, 100.0);
    assert_eq!(
        owner.host_object(rider).unwrap().contained_by,
        Some(container)
    );
    assert_eq!(
        owner.host_object(container).unwrap().contained_units(),
        vec![rider]
    );
}
#[test]
fn unfree_rider_destroy_flag_skips_body_death_and_its_instant_creation_effect() {
    let (mut owner, container, rider) = parsed_world("TransportContain", "", 0);
    owner
        .host_object_mut(container)
        .unwrap()
        .template_mut()
        .contain_module
        .destroy_riders_who_are_not_free_to_exit = true;
    owner.host_object_mut(rider).unwrap().cur_locomotor_name = None;
    kill(&mut owner, container);
    assert!(owner.host_object(rider).unwrap().status.destroyed);
    assert!(!owner.host_object(rider).unwrap().status.on_die_started);
    assert!(
        !owner
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "OwnedRiderDeathWitness")
    );
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn dead_rider_change_owner_removes_rider_by_direct_destruction() {
    let (mut owner, container, rider) = parsed_world("RiderChangeContain", "", 0);
    kill(&mut owner, container);
    assert!(owner.host_object(rider).unwrap().status.destroyed);
    assert!(!owner.host_object(rider).unwrap().status.on_die_started);
    assert!(
        owner
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert!(
        !owner
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "OwnedRiderDeathWitness")
    );
}

#[test]
fn cave_death_remaps_shared_pool_before_last_entrance_direct_destruction() {
    let (mut owner, first, rider) = parsed_world("CaveContain", "", 100);
    let (peer, peer_first, peer_rider) = parsed_world("CaveContain", "", 100);
    assert_eq!((first, rider), (peer_first, peer_rider));
    let index = owner.host_object(first).unwrap().cave_index;
    assert_eq!(owner.cave_system.index_of(first), Some(index));
    let second = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    assert_eq!(owner.cave_system.index_of(second), Some(index));
    assert_eq!(owner.cave_system.contained_for_index(index), vec![rider]);
    kill(&mut owner, first);
    assert_eq!(owner.host_object(rider).unwrap().health.current, 100.0);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, Some(second));
    assert_eq!(owner.cave_system.contained_for_index(index), vec![rider]);
    assert_eq!(peer.cave_system.index_of(first), Some(index));
    assert_eq!(peer.host_object(rider).unwrap().contained_by, Some(first));
    kill(&mut owner, second);
    assert!(owner.host_object(rider).unwrap().status.destroyed);
    assert!(!owner.host_object(rider).unwrap().status.on_die_started);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
    assert!(owner.cave_system.contained_for_index(index).is_empty());
    assert!(
        !owner
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "OwnedRiderDeathWitness")
    );
}

#[test]
fn cave_death_uses_registered_entrance_order_even_during_sale() {
    let (mut owner, first, rider) = parsed_world("CaveContain", "", 100);
    let (peer, peer_first, peer_rider) = parsed_world("CaveContain", "", 100);
    assert_eq!((first, rider), (peer_first, peer_rider));
    let index = owner.host_object(first).unwrap().cave_index;
    let second = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    let third = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(150.0, 0.0, 50.0))
        .unwrap();
    owner.host_object_mut(second).unwrap().status.sold = true;
    assert_eq!(owner.cave_system.cave_ids_for_index(index), vec![first, second, third]);

    kill(&mut owner, first);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, Some(second));
    assert!(!owner.host_object(rider).unwrap().status.destroyed);
    assert_eq!(owner.cave_system.contained_for_index(index), vec![rider]);
    assert_eq!(peer.host_object(rider).unwrap().contained_by, Some(first));
}
#[test]
fn selling_cave_orders_shared_riders_through_sold_entrance_without_immediate_removal() {
    let (mut owner, first, rider) = parsed_world("CaveContain", "", 100);
    let (peer, peer_first, peer_rider) = parsed_world("CaveContain", "", 100);
    assert_eq!((first, rider), (peer_first, peer_rider));
    let second = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    let index = owner.host_object(first).unwrap().cave_index;
    let before = owner.host_object(rider).unwrap().get_position();
    owner.on_selling_container_residual(second);
    assert_eq!(owner.cave_system.contained_for_index(index), vec![rider]);
    let passenger = owner.host_object(rider).unwrap();
    assert_eq!(passenger.contained_by, Some(first));
    assert_eq!(passenger.get_position(), before);
    assert_eq!(passenger.ai_state(), crate::game_logic::AIState::Exiting);
    assert_eq!(passenger.target, Some(second));
    assert_eq!(
        passenger.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI,
    );
    assert_eq!(peer.host_object(rider).unwrap().contained_by, Some(first));
    assert_ne!(peer.host_object(rider).unwrap().ai_state(), crate::game_logic::AIState::Exiting);

    owner.update_owned_passenger_exit(rider);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
    assert!(owner.cave_system.contained_for_index(index).is_empty());
    assert_eq!(owner.cave_system.cross_exits, 1);
    assert_eq!(peer.cave_system.contained_for_index(index), vec![rider]);
}

#[test]
fn selling_cave_leaves_passengers_without_ai_in_the_shared_tracker() {
    let (mut owner, cave, _) = parsed_world("CaveContain", "", 100);
    let index = owner.host_object(cave).unwrap().cave_index;
    let passenger = owner
        .create_object_for_player("OwnedRiderDeathWitness", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    assert!(!owner.host_object(passenger).unwrap().has_ai_update_interface());
    assert!(owner.cave_system.record_enter(index, passenger, cave, Team::USA).0);
    owner.set_fixture_object_contained_by(passenger, Some(cave));
    let position = owner.host_object(passenger).unwrap().get_position();
    owner.on_selling_container_residual(cave);
    assert!(owner.cave_system.is_in_network(index, passenger));
    assert_eq!(owner.host_object(passenger).unwrap().contained_by, Some(cave));
    assert_eq!(owner.host_object(passenger).unwrap().get_position(), position);
}


#[test]
fn cave_name_without_authored_module_is_not_a_cave() {
    let (mut owner, container, _) = parsed_world("TransportContain", "", 0);
    owner.host_object_mut(container).unwrap().template_name = "FakeCave".into();
    assert!(!owner.host_object(container).unwrap().is_cave_style_container());
}

#[test]
fn cave_exit_rejects_an_entrance_from_another_network() {
    let (mut owner, first, rider) = parsed_world("CaveContain", "", 100);
    let other = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    let index = owner.host_object(first).unwrap().cave_index;
    assert!(!owner.try_set_cave_index(other, index + 1));
    // Both trackers must be empty when the connection changes.
    assert!(owner.exit_cave_unit(rider, first));
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
    assert_eq!(owner.host_object(rider).unwrap().get_position(), owner.host_object(first).unwrap().get_position());
    assert!(owner.try_set_cave_index(other, index + 1));
    assert!(owner.cave_system.record_enter(index, rider, first, Team::USA).0);
    assert!(!owner.exit_cave_unit(rider, other));
    assert_eq!(owner.cave_system.contained_for_index(index), vec![rider]);
}

#[test]
fn cave_evacuation_removes_the_receiving_shared_roster_once() {
    let (mut owner, cave, rider) = parsed_world("CaveContain", "", 100);
    let (peer, peer_cave, peer_rider) = parsed_world("CaveContain", "", 100);
    assert_eq!((cave, rider), (peer_cave, peer_rider));
    let index = owner.host_object(cave).unwrap().cave_index;
    assert!(owner.evacuate_container_now(cave, false));
    assert!(owner.cave_system.contained_for_index(index).is_empty());
    assert_eq!(owner.cave_system.exits, 1);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
    assert_eq!(peer.cave_system.contained_for_index(index), vec![rider]);
    assert_eq!(peer.host_object(rider).unwrap().contained_by, Some(cave));
}

#[test]
fn cave_exit_accepts_same_index_after_entrance_unregistered() {
    let (mut owner, first, rider) = parsed_world("CaveContain", "", 100);
    let second = owner
        .create_object_for_player("OwnedContainDeath", 1, Vec3::new(100.0, 0.0, 50.0))
        .unwrap();
    let index = owner.host_object(first).unwrap().cave_index;
    assert!(!owner.cave_system.on_cave_destroyed(second).cave_in);
    assert_eq!(owner.cave_system.index_of(second), None);
    assert!(owner.exit_cave_unit(rider, second));
    assert!(owner.cave_system.contained_for_index(index).is_empty());
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
}
