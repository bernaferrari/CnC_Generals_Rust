//! Exact nullable controller observations, without a faction ownership repair.
use super::*;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "nullable-controller.ini".into(),
        "Object ControllerProbe\nKindOf = STRUCTURE\nEnergyProduction = 10\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    world.add_player(Player::new(1, Team::USA, "Controller", true));
    assert!(world.ensure_host_spawn_template("ControllerProbe"));
    world
}

#[test]
fn actual_no_team_admission_stays_controllerless_beside_a_same_faction_player_and_peer() {
    let mut own = world();
    let mut peer = world();
    let id = ObjectId(1);
    let object = Object::new(own.templates["ControllerProbe"].clone(), id, Team::USA);
    assert_eq!(own.try_add_object(object).unwrap(), id);
    let owned = peer
        .create_object_for_player("ControllerProbe", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        owned, id,
        "same raw ID in the receiving peer is an independent birth"
    );
    let object = own.objects.get(&id).unwrap();
    assert_eq!(object.team_instance_id, None);
    assert_eq!(object.owner_player_id, None);
    assert_eq!(own.player_owner_for_host_object(object), None);
    assert_eq!(own.player_owner_for_event(None, Team::USA), None);
    assert!(!own.is_object_locally_controlled(id));
    assert!(peer.is_object_locally_controlled(owned));
    assert_eq!(
        own.players[&1].power_produced, 0,
        "NoTeam never entered Player influence"
    );
    assert_eq!(
        peer.player_owner_for_host_object(&peer.objects[&owned]),
        Some(1)
    );
    assert_eq!(peer.players[&1].power_produced, 10);
}

#[test]
fn deferred_missing_controller_never_rebinds_to_another_same_faction_player() {
    let world = world();
    assert_eq!(world.player_owner_for_event(Some(999), Team::USA), None);
    assert_eq!(world.player_owner_for_event(None, Team::USA), None);
    assert_eq!(world.player_owner_for_event(Some(1), Team::USA), Some(1));
}

#[test]
fn actual_encoded_no_team_controller_continuation_does_not_acquire_a_peer_faction_owner() {
    let mut source = world();
    let id = ObjectId(1);
    let mut object = Object::new(source.templates["ControllerProbe"].clone(), id, Team::USA);
    // External completed-object ingress requires the same prebirth lifecycle
    // initialization used by the receiving factory, including an empty roster.
    object.bind_native_lifecycle(
        source.frame,
        &source.logic_random,
        source.native_hulk_max_lifetime_override,
    );
    source.try_add_object(object).unwrap();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let mut peer = world();
    let peer_id = peer
        .create_object_for_player("ControllerProbe", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(peer_id, id);
    for world in [&source, &restored] {
        let object = &world.objects[&id];
        assert_eq!(object.team_instance_id, None);
        assert_eq!(object.owner_player_id, None);
        assert_eq!(world.player_owner_for_host_object(object), None);
        assert!(!world.is_object_locally_controlled(id));
        assert_eq!(world.players[&1].power_produced, 0);
    }
    assert_eq!(
        peer.player_owner_for_host_object(&peer.objects[&id]),
        Some(1)
    );
    assert_eq!(peer.players[&1].power_produced, 10);
}

#[test]
fn exact_controller_identity_survives_inactive_and_different_faction_observations() {
    let mut receiving = world();
    let id = receiving
        .create_object_for_player("ControllerProbe", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        receiving.player_owner_for_host_object(&receiving.objects[&id]),
        Some(1)
    );
    receiving.players.get_mut(&1).unwrap().is_alive = false;
    assert_eq!(
        receiving.player_owner_for_host_object(&receiving.objects[&id]),
        Some(1)
    );
    assert_eq!(
        receiving.player_owner_for_event(Some(1), Team::USA),
        Some(1)
    );
    assert!(receiving.is_object_locally_controlled(id));
    receiving.objects.get_mut(&id).unwrap().team = Team::China;
    assert_eq!(
        receiving.player_owner_for_host_object(&receiving.objects[&id]),
        Some(1)
    );
    assert_eq!(
        receiving.player_owner_for_event(Some(1), Team::China),
        Some(1)
    );
    assert!(receiving.is_object_locally_controlled(id));
}
