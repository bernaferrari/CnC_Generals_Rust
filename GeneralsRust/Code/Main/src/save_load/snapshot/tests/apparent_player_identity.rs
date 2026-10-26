//! Actual retained player identities, including same-faction enemies.
use super::*;
fn wire_roundtrip(mut snapshot: WorldSnapshot) -> WorldSnapshot {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut save = XferSave::new(&mut bytes);
    snapshot.xfer(&mut save).unwrap();
    drop(save);
    let mut load = XferLoad::new(std::io::Cursor::new(bytes.into_inner()));
    let mut restored = WorldSnapshot::default();
    restored.xfer(&mut load).unwrap();
    restored
}
fn identity_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("IdentityTruck".into(), ThingTemplate::new("IdentityTruck"));
    world
        .templates
        .insert("IdentityTank".into(), ThingTemplate::new("IdentityTank"));
    world.add_player(Player::new(1, Team::USA, "First USA", true));
    world.add_player(Player::new(7, Team::USA, "Second USA", false));
    let id = world
        .create_object("IdentityTruck", Team::USA, Vec3::new(8., 0., 8.))
        .unwrap();
    (world, id)
}
#[test]
fn exact_disguise_player_survives_halfpoint_and_actual_world_roundtrip() {
    let (mut world, id) = identity_world();
    world
        .host_object_mut(id)
        .unwrap()
        .apply_disguise("IdentityTank", Team::USA, Some(7));
    assert_eq!(
        world.host_object(id).unwrap().disguise_pending_player_id,
        Some(7)
    );
    let builder = SnapshotBuilder::new();
    let snapshot = wire_roundtrip(builder.create_world_snapshot(&world).unwrap());
    let mut peer = GameLogic::new();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    for _ in 0..crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES {
        peer.host_object_mut(id).unwrap().tick_disguise_transition();
    }
    assert_eq!(peer.host_object(id).unwrap().disguise_as_player_id, Some(7));
    assert_eq!(
        peer.host_object(id).unwrap().disguise_pending_player_id,
        None
    );
    assert_eq!(
        world.host_object(id).unwrap().disguise_pending_player_id,
        Some(7)
    );
    peer.host_object_mut(id).unwrap().clear_disguise_instant();
    assert_eq!(peer.host_object(id).unwrap().disguise_as_player_id, None);
    assert_eq!(
        world.host_object(id).unwrap().disguise_pending_player_id,
        Some(7)
    );
}
#[test]
fn malformed_apparent_player_fails_before_replacing_running_world() {
    let (world, id) = identity_world();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    snapshot.object_disguises.push(ObjectDisguiseSnapshot {
        object_id: id,
        disguise_pending_template: "IdentityTank".into(),
        disguise_pending_team: 0,
        disguise_pending_player_id: Some(999),
        ..Default::default()
    });
    let (mut running, original) = identity_world();
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut running)
            .is_err()
    );
    assert!(running.host_object(original).is_some());
    assert_eq!(running.get_players().get(&7).unwrap().name, "Second USA");
}
#[test]
fn exact_original_garrison_player_roundtrips_independent_of_faction() {
    let (mut world, id) = identity_world();
    let object = world.host_object_mut(id).unwrap();
    object.object_type = crate::game_logic::ObjectType::Building;
    object.building_data = Some(crate::game_logic::buildings::BuildingData::new(
        crate::game_logic::buildings::BuildingType::Bunker,
    ));
    let data = object.building_data.as_mut().unwrap();
    data.original_team = Some(Team::USA);
    data.original_garrison_player_id = Some(7);
    let builder = SnapshotBuilder::new();
    let snapshot = wire_roundtrip(builder.create_world_snapshot(&world).unwrap());
    let mut peer = GameLogic::new();
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    let data = peer
        .host_object(id)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert_eq!(data.original_team, Some(Team::USA));
    assert_eq!(data.original_garrison_player_id, Some(7));
}

#[test]
fn schema33_rejects_previous_incomplete_identity_body_before_write() {
    use crate::save_load::{SaveLoadError, XferData, XferSave};
    let mut snapshot = WorldSnapshot::default();
    snapshot.version = 32;
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut output = XferSave::new(&mut bytes);
    assert!(matches!(
        snapshot.xfer(&mut output),
        Err(SaveLoadError::VersionMismatch { actual: 32, .. })
    ));
    drop(output);
    assert!(bytes.into_inner().is_empty());
}
