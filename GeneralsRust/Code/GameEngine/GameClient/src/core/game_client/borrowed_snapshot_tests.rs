use super::*;
use game_engine::Snapshot;

#[test]
fn receiving_snapshot_round_trips_while_foreign_client_is_registered() {
    crate::test_support::ensure_ambient_host();
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "receiving_snapshot_round_trips_while_foreign_client_is_registered",
    ) {
        return;
    }
    let mut owner = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    owner.set_frame(37);
    peer.set_frame(91);
    register_live_game_client(&mut peer);
    let file = tempfile::NamedTempFile::new().unwrap();
    let name = file.path().to_string_lossy().into_owned();
    let mut writer = game_engine::System::xfer_save::XferSave::new();
    writer.open(name.clone()).unwrap();
    owner.snapshot().xfer(&mut writer).unwrap();
    writer.close().unwrap();
    assert_eq!(peer.get_frame(), 91);
    let mut receiving = GameClient::new().unwrap();
    receiving.set_frame(777);
    let mut reader = game_engine::System::xfer_load::XferLoad::new();
    reader.open(name).unwrap();
    receiving.snapshot().xfer(&mut reader).unwrap();
    reader.close().unwrap();
    assert_eq!(receiving.get_frame(), 37);
    assert_eq!(owner.get_frame(), 37);
    assert_eq!(peer.get_frame(), 91);
    assert_eq!(live_game_client_frame(), Some(91));
    clear_live_game_client(&mut peer);
}

#[test]
fn moved_snapshot_owner_round_trips_same_id_without_changing_registered_peer() {
    crate::test_support::ensure_ambient_host();
    if crate::core::game_client::global_boundary_fixture::run_in_private_child(
        module_path!(),
        "moved_snapshot_owner_round_trips_same_id_without_changing_registered_peer",
    ) {
        return;
    }
    let mut factory = game_engine::common::thing::ThingFactory::new();
    let template = factory.new_template("BorrowedSnapshotSameId");
    let mut source = GameClient::new().unwrap();
    let source_id = source.create_drawable_from_template(&template).unwrap();
    source.bind_drawable_to_object(source_id, 42).unwrap();
    source.find_drawable_by_id_mut(source_id).unwrap()
        .set_position(Vector3::new(3.0, 5.0, 7.0));
    source.set_frame(37);
    let mut source = Box::new(source);

    let mut peer = Box::new(GameClient::new().unwrap());
    let peer_id = peer.create_drawable_from_template(&template).unwrap();
    peer.bind_drawable_to_object(peer_id, 42).unwrap();
    peer.find_drawable_by_id_mut(peer_id).unwrap()
        .set_position(Vector3::new(11.0, 13.0, 17.0));
    peer.set_frame(91);
    assert_eq!(source_id, peer_id);
    register_live_game_client(&mut peer);

    let old_factory = game_engine::common::thing::get_thing_factory()
        .unwrap()
        .replace(factory);
    let file = tempfile::NamedTempFile::new().unwrap();
    let name = file.path().to_string_lossy().into_owned();
    let mut writer = game_engine::System::xfer_save::XferSave::new();
    writer.open(name.clone()).unwrap();
    source.snapshot().xfer(&mut writer).unwrap();
    writer.close().unwrap();
    drop(source);

    let mut receiving = GameClient::new().unwrap();
    let mut reader = game_engine::System::xfer_load::XferLoad::new();
    reader.open(name).unwrap();
    receiving.snapshot().xfer(&mut reader).unwrap();
    reader.close().unwrap();
    *game_engine::common::thing::get_thing_factory().unwrap() = old_factory;

    assert_eq!(receiving.get_frame(), 37);
    let restored_id = receiving.get_drawable_for_object(42).unwrap();
    assert_eq!(restored_id, source_id);
    assert_eq!(
        receiving.find_drawable_by_id(restored_id).unwrap().get_position(),
        Vector3::new(3.0, 5.0, 7.0)
    );
    assert_eq!(peer.get_frame(), 91);
    assert_eq!(peer.get_drawable_for_object(42), Some(peer_id));
    assert_eq!(
        peer.find_drawable_by_id(peer_id).unwrap().get_position(),
        Vector3::new(11.0, 13.0, 17.0)
    );
    assert_eq!(live_game_client_frame(), Some(91));
    clear_live_game_client(&mut peer);
}
