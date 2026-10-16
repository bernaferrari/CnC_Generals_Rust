use super::*;
use game_engine::Snapshot;

#[test]
fn receiving_snapshot_round_trips_while_foreign_client_is_registered() {
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
