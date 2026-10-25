//! CPP OverlordContain157/188/205: physical payload and redirection phases.
use super::*;
use crate::command_system::{CommandType, GameCommand, ModifierKeys};
use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_BUNKER;

fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    world_with_authored_deaths(None)
}
fn world_with_authored_deaths(
    definitions: Option<(ThingTemplate, ThingTemplate)>,
) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut owner = GameLogic::new();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner.add_player(Player::new(1, Team::China, "OverlordOwner", true));
    let mut infantry = ThingTemplate::new("PhysicalBunkerInfantry");
    infantry
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    infantry.transport_slot_count = Some(1);
    infantry.locomotor_name = Some("BasicHumanLocomotor".into());
    infantry.set_authored_ai_update_interface(Some(true));
    infantry.authored_locomotor_sets = Some(vec![
        crate::game_logic::host_upgrade_module_residuals::AuthoredLocomotorSet {
            kind: crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Normal,
            members: vec!["BasicHumanLocomotor".into()],
        },
    ]);
    if let Some((authored, payload)) = definitions {
        infantry = authored;
        owner.templates.insert(payload.name.clone(), payload);
    }
    owner.templates.insert(infantry.name.clone(), infantry);
    let unit = owner
        .create_object_for_player("PhysicalBunkerInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let mut hull = ThingTemplate::new("ChinaTankOverlord");
    hull.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    owner.templates.insert(hull.name.clone(), hull);
    let hull = owner
        .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    owner.apply_upgrade_to_object(hull, UPGRADE_OVERLORD_BUNKER);
    let payload = owner
        .host_object(hull)
        .unwrap()
        .overlord_portable_occupant
        .expect("actual upgrade payload");
    (owner, unit, hull, payload)
}
fn world_with_authored_death_source(source: &str) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut owner = GameLogic::new();
    owner.world_min = Vec3::ZERO;
    owner.world_max = Vec3::splat(1000.0);
    owner
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "OwnedOverlordDeath.ini".into(),
                source.into(),
            )])
            .unwrap(),
        )
        .unwrap();
    owner.add_player(Player::new(1, Team::China, "OverlordOwner", true));
    let unit = owner
        .create_object_for_player("PhysicalBunkerInfantry", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let hull = owner
        .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    owner.apply_upgrade_to_object(hull, UPGRADE_OVERLORD_BUNKER);
    let payload = owner
        .host_object(hull)
        .unwrap()
        .overlord_portable_occupant
        .expect("actual upgrade payload");
    (owner, unit, hull, payload)
}

fn enter(owner: &mut GameLogic, unit: ObjectId, hull: ObjectId) {
    assert!(owner.can_unit_enter_normal_target(unit, hull));
    owner.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: hull },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![unit],
        modifier_keys: ModifierKeys::default(),
    });
    owner.process_commands();
    assert_eq!(owner.host_object(unit).unwrap().target, Some(hull));
    owner
        .update_ai(&[unit, hull], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
}
#[test]
fn overlord_enter_uses_actual_payload_roster_and_preserves_same_id_peer() {
    let (mut owner, unit, hull, payload) = world();
    let (peer, peer_unit, peer_hull, peer_payload) = world();
    assert_eq!((unit, hull, payload), (peer_unit, peer_hull, peer_payload));
    assert_eq!(owner.host_object(payload).unwrap().max_transport, 5);
    enter(&mut owner, unit, hull);
    assert_eq!(
        owner.host_object(hull).unwrap().contained_units(),
        vec![payload]
    );
    assert_eq!(
        owner.host_object(payload).unwrap().contained_units(),
        vec![unit]
    );
    assert_eq!(owner.host_object(unit).unwrap().contained_by, Some(payload));
    assert_eq!(owner.owned_script_containment(hull).unwrap().count, 1);
    assert_eq!(owner.owned_script_containment(hull).unwrap().max, 5);
    assert!(
        peer.host_object(payload)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    owner.host_object_mut(hull).unwrap().status.disabled_subdued = true;
    assert!(!owner.can_unit_enter_normal_target(peer_unit, hull));
    assert!(!owner.evacuate_container_now(hull, false));
    owner.host_object_mut(hull).unwrap().status.disabled_subdued = false;
    assert!(owner.evacuate_container_now(hull, false));
    assert!(
        owner
            .host_object(payload)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.host_object(unit).unwrap().contained_by, None);
    assert_eq!(
        owner.host_object(hull).unwrap().contained_units(),
        vec![payload]
    );
}
#[test]
fn overlord_delete_detaches_payload_and_riders_before_physical_removal() {
    let (mut owner, unit, hull, payload) = world();
    enter(&mut owner, unit, hull);
    owner.destroy_object(hull);
    assert!(owner.host_object(hull).unwrap().status.destroyed);
    assert!(!owner.host_object(hull).unwrap().overlord_redirection_active);
    assert!(
        owner
            .host_object(hull)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert!(owner.host_object(payload).unwrap().is_alive());
    assert!(
        owner
            .host_object(payload)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.host_object(payload).unwrap().contained_by, None);
    assert!(owner.host_object(unit).unwrap().is_alive());
    assert_eq!(owner.host_object(unit).unwrap().contained_by, None);
}
#[test]
fn overlord_redirection_phase_survives_owned_lifecycle_codec() {
    let (mut owner, _, hull, payload) = world();
    let saved = owner
        .host_object(hull)
        .unwrap()
        .entity_lifecycle_envelope()
        .expect("valid lifecycle capture");
    let bytes = saved.encode();
    let decoded = gamelogic::world::entities::EntityLifecycleEnvelope::decode(&bytes).unwrap();
    owner
        .host_object_mut(hull)
        .unwrap()
        .overlord_redirection_active = false;
    owner
        .host_object_mut(hull)
        .unwrap()
        .overlord_portable_occupant = None;
    owner
        .host_object_mut(hull)
        .unwrap()
        .entity_apply_lifecycle_envelope(&decoded)
        .unwrap();
    assert_eq!(owner.redirected_containment_owner(hull), payload);
    let outer = owner.host_object_mut(hull).unwrap();
    outer.overlord_redirection_active = false;
    let saved_inactive = outer
        .entity_lifecycle_envelope()
        .expect("valid lifecycle capture");
    outer.overlord_redirection_active = true;
    outer
        .entity_apply_lifecycle_envelope(&saved_inactive)
        .unwrap();
    assert_eq!(owner.redirected_containment_owner(hull), hull);
    assert_eq!(owner.owned_script_containment(hull).unwrap().max, 1);
}
#[test]
fn overlord_on_die_deactivates_then_completes_installed_payload_before_removal() {
    let (mut owner, unit, hull, payload) = world();
    enter(&mut owner, unit, hull);
    assert!(
        owner
            .objects
            .get_mut(&hull)
            .unwrap()
            .kill(&mut owner.health_events)
    );
    owner.mark_object_for_destruction(hull, None);
    assert!(!owner.host_object(hull).unwrap().overlord_redirection_active);
    assert_eq!(
        owner.host_object(hull).unwrap().overlord_portable_occupant,
        None
    );
    assert!(
        owner
            .host_object(hull)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert!(!owner.host_object(payload).unwrap().is_alive());
    assert!(
        owner
            .host_object(payload)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert_eq!(owner.host_object(payload).unwrap().contained_by, None);
    assert_eq!(owner.host_object(unit).unwrap().contained_by, None);
    assert_eq!(
        owner.host_object(unit).unwrap().health.current,
        50.0,
        "retail bunker DamagePercentToUnits=50% is applied synchronously"
    );
    // All three identities are still installed at this observation point.
    assert!(owner.host_object(hull).is_some());
    assert!(owner.host_object(payload).is_some());
    assert!(owner.host_object(unit).is_some());
}

#[test]
fn authored_payload_contain_death_precedes_instant_death_creation_callback() {
    // Admit actual immutable OCLs before binding receiving Die definitions.
    for name in ["RiderDeathWitness", "PayloadDeathWitness"] {
        let mut list = gamelogic::object_creation_list::store::ObjectCreationList::new();
        list.add_nugget(std::sync::Arc::new(
            gamelogic::object_creation_list::GenericObjectCreationNugget {
                names: vec![name.into()],
                name_are_objects: true,
                ..Default::default()
            },
        ));
        gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(name, list);
    }
    let authored_source = r#"
Object PhysicalBunkerInfantry
  KindOf = INFANTRY SELECTABLE CAN_ATTACK
  TransportSlotCount = 1
  Locomotor = SET_NORMAL BasicHumanLocomotor
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
  Behavior = InstantDeathBehavior ModuleTag_Death
    OCL = RiderDeathWitness
  End
End
Object ChinaTankOverlord
  KindOf = VEHICLE CAN_ATTACK
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1000
    InitialHealth = 1000
  End
End
Object ChinaTankOverlordBattleBunker
  KindOf = VEHICLE CAN_ATTACK
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = TransportContain ModuleTag_Contain
    Slots = 5
    AllowInsideKindOf = INFANTRY
    DamagePercentToUnits = 100%
  End
  Behavior = InstantDeathBehavior ModuleTag_Death
    OCL = PayloadDeathWitness
  End
End
Object RiderDeathWitness
 KindOf = IMMOBILE
 Body = ActiveBody WitnessBody
  MaxHealth = 1
  InitialHealth = 1
 End
End
Object PayloadDeathWitness
 KindOf = IMMOBILE
 Body = ActiveBody WitnessBody
  MaxHealth = 1
  InitialHealth = 1
 End
End
"#;
    let (mut owner, unit, hull, child) = world_with_authored_death_source(authored_source);
    let peer_source = authored_source.replace("100%", "25%");
    let (peer, _, _, _) = world_with_authored_death_source(&peer_source);
    assert_eq!(
        peer.templates["ChinaTankOverlordBattleBunker"]
            .contain_module
            .damage_percent_to_units,
        Some(0.25),
        "same-name admitted payloads retain each world's authored damage percentage"
    );
    assert_eq!(
        owner.templates["ChinaTankOverlordBattleBunker"]
            .contain_module
            .damage_percent_to_units,
        Some(1.0)
    );
    enter(&mut owner, unit, hull);
    assert!(
        owner
            .objects
            .get_mut(&hull)
            .unwrap()
            .kill(&mut owner.health_events)
    );
    owner.mark_object_for_destruction(hull, None);
    let rider_witness = owner
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "RiderDeathWitness")
        .expect("actual rider InstantDeath OCL")
        .id;
    let payload_witness = owner
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "PayloadDeathWitness")
        .expect("actual payload InstantDeath OCL")
        .id;
    assert!(
        rider_witness.0 < payload_witness.0,
        "child contain onDie must finish before its later InstantDeath effect"
    );
    assert_eq!(owner.host_object(unit).unwrap().contained_by, None);
    assert!(
        owner
            .host_object(child)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn malformed_overlord_phase_restore_keeps_running_owner_and_same_id_peer() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    let (mut owner, unit, hull, payload) = world();
    let (peer, peer_unit, peer_hull, peer_payload) = world();
    assert_eq!((unit, hull, payload), (peer_unit, peer_hull, peer_payload));
    enter(&mut owner, unit, hull);
    let mut tail = capture_lifecycle_tail(&owner).expect("valid lifecycle capture");
    let envelope = tail
        .envelopes
        .iter_mut()
        .find(|entry| entry.entity_id == hull.0)
        .unwrap();
    let phase = envelope
        .module_states
        .iter_mut()
        .find(|entry| entry.tag == "OverlordAddon")
        .expect("live codec captures actual phase");
    phase.payload = vec![0xFF];
    assert!(apply_lifecycle_tail_to_host(&tail, &mut owner).is_err());
    assert_eq!(owner.redirected_containment_owner(hull), payload);
    assert_eq!(owner.host_object(unit).unwrap().contained_by, Some(payload));
    assert_eq!(peer.redirected_containment_owner(peer_hull), peer_payload);
    assert_eq!(peer.host_object(peer_unit).unwrap().contained_by, None);
}

#[test]
fn active_overlord_restore_rejects_wrong_child_and_admits_saved_links() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    let (mut owner, unit, hull, payload) = world();
    let (mut peer, peer_unit, peer_hull, peer_payload) = world();
    assert_eq!((unit, hull, payload), (peer_unit, peer_hull, peer_payload));
    enter(&mut owner, unit, hull);
    let foreign = peer
        .create_object_for_player("PhysicalBunkerInfantry", 1, Vec3::ZERO)
        .unwrap();
    assert!(owner.host_object(foreign).is_none());
    let saved = capture_lifecycle_tail(&owner).expect("valid lifecycle capture");
    for wrong_child in [None, Some(hull), Some(unit), Some(foreign)] {
        let mut invalid = saved.clone();
        let mut malformed = owner.host_object(hull).unwrap().clone();
        malformed.overlord_portable_occupant = wrong_child;
        *invalid
            .envelopes
            .iter_mut()
            .find(|entry| entry.entity_id == hull.0)
            .unwrap() = malformed
            .entity_lifecycle_envelope()
            .expect("valid lifecycle capture");
        assert!(apply_lifecycle_tail_to_host(&invalid, &mut owner).is_err());
        assert_eq!(owner.redirected_containment_owner(hull), payload);
        assert_eq!(owner.host_object(payload).unwrap().contained_by, Some(hull));
        assert_eq!(owner.host_object(unit).unwrap().contained_by, Some(payload));
        assert_eq!(peer.redirected_containment_owner(peer_hull), peer_payload);
        assert_eq!(peer.host_object(peer_unit).unwrap().contained_by, None);
    }
    // Detached restore candidates use the actual saved containment links.
    owner.host_object_mut(hull).unwrap().occupants.clear();
    owner.host_object_mut(payload).unwrap().contained_by = None;
    apply_lifecycle_tail_to_host(&saved, &mut owner).unwrap();
    assert_eq!(owner.redirected_containment_owner(hull), payload);
    assert!(
        owner
            .host_object(hull)
            .unwrap()
            .contained_units()
            .contains(&payload)
    );
    assert_eq!(owner.host_object(payload).unwrap().contained_by, Some(hull));
    assert_eq!(peer.host_object(peer_unit).unwrap().contained_by, None);
}
