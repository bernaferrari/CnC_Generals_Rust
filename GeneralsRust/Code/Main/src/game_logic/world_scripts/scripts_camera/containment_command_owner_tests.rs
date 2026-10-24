//! AIUpdate privateEnter/Exit/Evacuate through the actual borrowed script walk.
use super::named_command_test_support::{execute, world};
use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn transport(world: &mut GameLogic) -> ObjectId {
    let mut template = ThingTemplate::new("OwnedExitTransport");
    template.add_kind_of(KindOf::Vehicle).set_health(300.0);
    template.set_authored_ai_update_interface(Some(true));
    template.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(1),
        admission: ContainAdmission::InfantryOnly,
        ..ContainModuleMetadata::default()
    };
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("OwnedExitTransport", 1, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    world.host_object_mut(id).unwrap().name = "NamedTransport".into();
    id
}

fn load(world: &mut GameLogic, passenger: ObjectId, container: ObjectId) {
    assert!(
        world
            .host_object_mut(container)
            .unwrap()
            .add_occupant(passenger)
    );
    let (rider, fixture_partition_rider) = world.host_object_and_partition_mut(passenger).unwrap();
    rider.set_contained_by_with_partition_owner(Some(container), true, fixture_partition_rider);
    rider.set_ai_state(
        AIState::Garrisoned,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    rider.set_status_stealthed(true);
    rider.set_status_detected(false);
}

fn evacuate(world: &mut GameLogic) {
    assert_eq!(
        execute(
            world,
            &[(ScriptActionType::NamedExitAll, "NamedTransport", "")]
        ),
        1
    );
}

#[test]
fn enter_rejected_noncontainer_preserves_ai_after_script_preparation() {
    let (mut a, id, target) = world();
    let (b, foreign, _) = world();
    assert_eq!(id, foreign);
    let old = Vec3::new(250.0, 0.0, 250.0);
    let (unit, attack_events) = a.host_object_and_attack_events_mut(id).unwrap();
    unit.set_target(
        Some(target),
        attack_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    unit.set_ai_state(
        AIState::AttackMoving,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    unit.pending_move = Some(old);
    unit.last_command_source = 99;
    assert_eq!(
        execute(
            &mut a,
            &[(
                ScriptActionType::NamedEnterNamed,
                "NamedUnit",
                "NamedTarget"
            )]
        ),
        1
    );
    let unit = a.host_object(id).unwrap();
    assert_eq!(unit.ai_state, AIState::AttackMoving);
    assert_eq!(unit.target, Some(target));
    assert_eq!(unit.pending_move, Some(old));
    assert_eq!(unit.last_command_source, 99);
    assert_eq!(unit.formation_id, 0);
    assert_eq!(b.host_object(foreign).unwrap().formation_id, 19);
}

#[test]
fn enter_command_ignores_capacity_but_arrival_rechecks_it() {
    let (mut a, original, _) = world();
    // CPP ThingTemplate defaults TransportSlotCount to zero. A normal
    // transport rider must explicitly author a positive slot count even when
    // the command bypasses the current capacity check.
    a.templates
        .get_mut("OwnedNamedInfantry")
        .unwrap()
        .transport_slot_count = Some(1);
    a.host_object_mut(original).unwrap().name.clear();
    let id = a
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    a.host_object_mut(id).unwrap().name = "NamedUnit".into();
    let container = transport(&mut a);
    let occupant = a
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::ZERO)
        .unwrap();
    load(&mut a, occupant, container);
    assert!(!a.can_unit_enter_normal_target(id, container));
    assert!(a.can_unit_enter_script_target(id, container));
    assert_eq!(
        execute(
            &mut a,
            &[(
                ScriptActionType::NamedEnterNamed,
                "NamedUnit",
                "NamedTransport"
            )]
        ),
        1
    );
    assert_eq!(a.host_object(id).unwrap().ai_state, AIState::Entering);
    a.set_fixture_object_position(id, Vec3::new(70.0, 0.0, 70.0));
    a.update_support_states(&[id], 1.0 / 30.0);
    assert_eq!(a.host_object(id).unwrap().contained_by, None);
    assert_eq!(
        a.host_object(container).unwrap().contained_units(),
        vec![occupant]
    );
}

#[test]
fn false_evacuation_orders_passenger_without_detection_or_instant_unload() {
    let (mut a, id, _) = world();
    let (mut b, foreign, _) = world();
    let container = transport(&mut a);
    let foreign_container = transport(&mut b);
    assert_eq!(container, foreign_container);
    load(&mut a, id, container);
    load(&mut b, foreign, foreign_container);
    evacuate(&mut a);
    let rider = a.host_object(id).unwrap();
    assert_eq!(rider.ai_state, AIState::Exiting);
    assert_eq!(rider.target, Some(container));
    assert_eq!(rider.contained_by, Some(container));
    assert!(rider.status.stealthed);
    assert!(!rider.status.detected);
    assert_eq!(
        rider.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        b.host_object(foreign).unwrap().ai_state,
        AIState::Garrisoned
    );
    a.update_support_states(&[id], 1.0 / 30.0);
    assert_eq!(a.host_object(id).unwrap().contained_by, None);
    assert!(
        a.host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(
        b.host_object(foreign).unwrap().contained_by,
        Some(foreign_container)
    );
}

#[test]
fn subdued_and_missing_ai_passengers_do_not_receive_exit() {
    let (mut a, id, _) = world();
    let container = transport(&mut a);
    load(&mut a, id, container);
    a.host_object_mut(container)
        .unwrap()
        .set_status_disabled_subdued(true);
    evacuate(&mut a);
    assert_eq!(a.host_object(id).unwrap().ai_state, AIState::Garrisoned);
    assert_eq!(a.host_object(id).unwrap().contained_by, Some(container));
}

#[test]
fn pending_exit_survives_world_snapshot_and_busy_door() {
    let (mut a, id, _) = world();
    let container = transport(&mut a);
    load(&mut a, id, container);
    a.frame = 10;
    a.host_object_mut(container).unwrap().frame_exit_not_busy = 20;
    evacuate(&mut a);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&a).unwrap();
    let (mut b, _, _) = world();
    transport(&mut b);
    builder.restore_from_snapshot(&snapshot, &mut b).unwrap();
    for owner in [&mut a, &mut b] {
        assert_eq!(owner.host_object(id).unwrap().ai_state, AIState::Exiting);
        assert_eq!(owner.host_object(id).unwrap().target, Some(container));
        assert_eq!(
            owner.host_object(id).unwrap().last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
        owner.frame = 19;
        owner.update_support_states(&[id], 1.0 / 30.0);
        assert_eq!(owner.host_object(id).unwrap().contained_by, Some(container));
        owner.frame = 20;
        owner.update_support_states(&[id], 1.0 / 30.0);
        assert_eq!(owner.host_object(id).unwrap().contained_by, None);
    }
    assert_eq!(
        a.host_object(id).unwrap().ai_state,
        b.host_object(id).unwrap().ai_state
    );
    assert_eq!(
        a.host_object(id).unwrap().get_position(),
        b.host_object(id).unwrap().get_position()
    );
}

#[test]
fn malformed_exit_goal_cannot_remove_a_different_containers_passenger() {
    let (mut a, id, _) = world();
    let container = transport(&mut a);
    load(&mut a, id, container);
    evacuate(&mut a);
    {
        let (object, attack_events) = a.host_object_and_attack_events_mut(id).unwrap();
        object.set_target(
            Some(id),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
    a.update_support_states(&[id], 1.0 / 30.0);
    assert_eq!(a.host_object(id).unwrap().contained_by, Some(container));
    assert_eq!(
        a.host_object(container).unwrap().contained_units(),
        vec![id]
    );
}

#[test]
fn false_evacuation_preserves_stealth_garrison_detection_flags() {
    let (mut owner, _, _) = world();
    let container = transport(&mut owner);
    let mut template = owner.templates["OwnedNamedInfantry"].clone();
    template.name = "HiddenExitPassenger".into();
    template.add_kind_of(KindOf::StealthGarrison);
    owner.templates.insert(template.name.clone(), template);
    let rider = owner
        .create_object_for_player("HiddenExitPassenger", 1, Vec3::ZERO)
        .unwrap();
    load(&mut owner, rider, container);
    evacuate(&mut owner);
    let object = owner.host_object(rider).unwrap();
    assert!(object.status.stealthed);
    assert!(!object.status.detected);
    assert_eq!(object.ai_state, AIState::Exiting);
}

#[test]
fn passenger_exit_waits_for_the_previous_riders_transport_door() {
    let (mut owner, first, _) = world();
    let container = transport(&mut owner);
    owner
        .host_object_mut(container)
        .unwrap()
        .install_humvee_transport();
    let second = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::ZERO)
        .unwrap();
    load(&mut owner, first, container);
    load(&mut owner, second, container);
    evacuate(&mut owner);
    assert_eq!(
        owner.host_object(container).unwrap().contained_units(),
        vec![first, second]
    );
    owner.frame = 10;
    owner.update_support_states(&[first, second], 1.0 / 30.0);
    assert_eq!(owner.host_object(first).unwrap().contained_by, None);
    assert_eq!(
        owner.host_object(second).unwrap().contained_by,
        Some(container)
    );
    assert_eq!(
        owner.host_object(second).unwrap().ai_state,
        AIState::Exiting
    );
    let next = owner.host_object(container).unwrap().frame_exit_not_busy;
    assert!(next > 10);
    owner.frame = next;
    owner.update_support_states(&[second], 1.0 / 30.0);
    assert_eq!(owner.host_object(second).unwrap().contained_by, None);
}

#[test]
fn exit_request_admits_landing_only_at_chinook_update_and_waits_until_landed() {
    use crate::game_logic::host_combat_chinook::{
        HostChinookAI, HostChinookAIState, HostChinookFlightStatus,
    };
    let (mut owner, rider, _) = world();
    let container = transport(&mut owner);
    let (hull, fixture_partition_hull) = owner.host_object_and_partition_mut(container).unwrap();
    hull.set_position_with_partition_owner(
        Vec3::new(70.0, 30.0, 70.0),
        0,
        &mut Default::default(),
        fixture_partition_hull,
    );
    hull.chinook_ai = Some(HostChinookAI::new_vanilla([70.0, 70.0, 30.0]));
    load(&mut owner, rider, container);
    evacuate(&mut owner);
    assert_eq!(
        owner
            .host_object(container)
            .unwrap()
            .chinook_ai
            .as_ref()
            .unwrap()
            .state,
        HostChinookAIState::Idle
    );
    owner.update_support_states(&[rider], 1.0 / 30.0);
    assert_eq!(
        owner.host_object(rider).unwrap().contained_by,
        Some(container)
    );
    owner.tick_chinook_ai(1.0 / 30.0);
    let ai = owner
        .host_object(container)
        .unwrap()
        .chinook_ai
        .as_ref()
        .unwrap();
    assert!(ai.wanting_enter_or_exit);
    assert_eq!(ai.state, HostChinookAIState::Landing);
    owner
        .host_object_mut(container)
        .unwrap()
        .chinook_ai
        .as_mut()
        .unwrap()
        .flight_status = HostChinookFlightStatus::Landed;
    owner.update_support_states(&[rider], 1.0 / 30.0);
    assert_eq!(owner.host_object(rider).unwrap().contained_by, None);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn enter_group_preparation_does_not_write_foreign_core_object_with_same_id() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "enter_group_preparation_does_not_write_foreign_core_object_with_same_id",
        || {
            let (mut world, id, _) = world();
            let mut foreign = gamelogic::object::Object::new_raw(
                std::sync::Arc::new(gamelogic::common::DefaultThingTemplate::new(
                    "ForeignCoreOwner".into(),
                )),
                id.0,
                gamelogic::common::ObjectStatusMaskType::NONE,
                None,
            );
            foreign.enter_group(&gamelogic::ai::AIGroup::new(913));
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(foreign));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign);
            assert_eq!(
                execute(
                    &mut world,
                    &[(
                        ScriptActionType::NamedEnterNamed,
                        "NamedUnit",
                        "NamedTarget"
                    )]
                ),
                1
            );
            assert_eq!(world.host_object(id).unwrap().formation_id, 0);
            assert_eq!(
                foreign.read().unwrap().get_group_id(),
                Some(913),
                "only the driving host owner leaves its group"
            );
            gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(id.0);
        },
    );
}
