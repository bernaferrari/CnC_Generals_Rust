//! Installed containment onDie ordering, independent of physical destruction.
use super::*;
use crate::game_logic::object::Object;

fn parsed_world(
    container_module: &str,
    death_module: &str,
    percent: u32,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    let input = format!(
        r#"
Object OwnedContainDeath
  KindOf = VEHICLE CAN_ATTACK
  Body = ActiveBody BodyTag
    MaxHealth = 200
  End
  Behavior = {container_module} ContainTag
    Slots = 5
    DamagePercentToUnits = {percent}%
    AllowInsideKindOf = INFANTRY
  End
  {death_module}
End
Object OwnedContainRider
  KindOf = INFANTRY CAN_ATTACK
  TransportSlotCount = 1
  Locomotor = SET_NORMAL BasicHumanLocomotor
  Body = ActiveBody BodyTag
    MaxHealth = 100
  End
  Behavior = AIUpdateInterface AiTag
  End
  Behavior = InstantDeathBehavior DeathTag
    OCL = OwnedRiderDeathWitness
  End
End
"#
    );
    parser
        .parse_ini_content(&input, "OwnedContainDeath.ini")
        .unwrap();
    let mut owner = GameLogic::new();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner.add_player(Player::new(1, Team::USA, "ContainOwner", true));
    for name in ["OwnedContainDeath", "OwnedContainRider"] {
        let template = GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).unwrap(),
            None,
        );
        owner.templates.insert(name.into(), template);
    }
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
    owner
        .host_object_mut(rider)
        .unwrap()
        .set_contained_by(Some(container));
    (owner, container, rider)
}
fn kill(owner: &mut GameLogic, id: ObjectId) {
    assert!(
        owner
            .objects
            .get_mut(&id)
            .unwrap()
            .kill(&mut owner.health_events)
    );
    owner.mark_object_for_destruction(id, None);
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
    assert_eq!(passenger.ai_state, AIState::Moving);
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
    owner
        .host_object_mut(second)
        .unwrap()
        .set_contained_by(Some(container));
    kill(&mut owner, container);
    let container_witness = owner
        .host_objects()
        .values()
        .find(|object| object.template_name == "OwnedContainerDeathWitness")
        .unwrap()
        .id;
    let rider_witnesses: Vec<_> = owner
        .host_objects()
        .values()
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
            .values()
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
            .values()
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
            .values()
            .any(|object| object.template_name == "OwnedRiderDeathWitness")
    );
}
