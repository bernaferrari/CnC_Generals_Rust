use super::*;

#[test]
fn actual_presentation_shell_preserves_the_admitted_client_frame() {
    if crate::core::game_client::global_boundary_fixture::run_in_private_child(
        module_path!(),
        "actual_presentation_shell_preserves_the_admitted_client_frame",
    ) {
        return;
    }
    // The real shell service has an initialization contract even without devices.
    // A fresh standalone public shell routes to the real local player queue.
    gamelogic::commands::command_queue::get_command_queue_manager()
        .lock()
        .expect("standalone command manager")
        .initialize_player(0)
        .expect("fresh local player command queue");
    let mut client = GameClient::new().unwrap();
    client.window_manager_owner().shell_handle().init().unwrap();
    client.initialized = true;
    client.set_frame(37);
    client.update_presentation_shell(0.0).unwrap();
    assert_eq!(client.get_frame(), 37);
    client.update_presentation_shell(0.0).unwrap();
    assert_eq!(
        client.get_frame(),
        37,
        "extra presentation cannot create a new client phase"
    );
    let mut peer = GameClient::new().unwrap();
    peer.window_manager_owner().shell_handle().init().unwrap();
    peer.initialized = true;
    peer.set_frame(900);
    peer.update_presentation_shell(0.0).unwrap();
    assert_eq!(peer.get_frame(), 900);
    assert_eq!(client.get_frame(), 37);
    client.set_frame(38);
    client.update_presentation_shell(0.0).unwrap();
    assert_eq!(client.get_frame(), 38);
    let mut saved = Vec::new();
    client
        .xfer(&mut game_engine::common::system::xfer_save::XferSave::new(
            Cursor::new(&mut saved),
            3,
        ))
        .unwrap();
    let mut restored = GameClient::new().unwrap();
    restored.initialized = true;
    restored.set_frame(901);
    restored
        .xfer(&mut game_engine::common::system::xfer_load::XferLoad::new(
            Cursor::new(saved),
            3,
        ))
        .unwrap();
    assert_eq!(restored.get_frame(), 38);
    restored.update_presentation_shell(0.0).unwrap();
    assert_eq!(
        restored.get_frame(),
        38,
        "native post-load presentation retains saved own Client clock"
    );
    assert_eq!(peer.get_frame(), 900);
    let mut no_companion = GameClient::new().unwrap();
    no_companion.initialized = true;
    no_companion.set_frame(902);
    no_companion.restore_from_xfer_bytes(&[]).unwrap();
    assert_eq!(no_companion.get_frame(), 902);
    no_companion.update_presentation_shell(0.0).unwrap();
    assert_eq!(
        no_companion.get_frame(),
        902,
        "an omitted Client companion cannot manufacture frame0 or next Logic frame"
    );
}
