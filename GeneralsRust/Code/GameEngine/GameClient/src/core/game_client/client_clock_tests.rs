use super::*;

#[test]
fn actual_presentation_shell_preserves_the_admitted_client_frame() {
    // The real shell service has an initialization contract even without devices.
    get_shell().init().unwrap();
    let mut client = GameClient::new().unwrap();
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
