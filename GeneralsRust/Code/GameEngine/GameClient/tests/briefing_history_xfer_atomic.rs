use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::callbacks::diplomacy::{
    get_briefing_text_list, update_diplomacy_briefing_text,
};

fn add_briefing(client: &mut GameClient, text: &str) {
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            effects.with_diplomacy(|owner| {
                update_diplomacy_briefing_text(text, false, manager, owner)
            });
        });
    });
}

fn history(client: &GameClient) -> Vec<String> {
    get_briefing_text_list(client.diplomacy())
}

#[test]
fn public_xfer_briefing_history_decodes_before_replacing_each_client() {
    let mut source = GameClient::new().unwrap();
    add_briefing(&mut source, "SCRIPT:SourceOne");
    add_briefing(&mut source, "SCRIPT:SourceTwo");
    let bytes = source.capture_xfer_bytes().unwrap();

    let mut receiver = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    add_briefing(&mut receiver, "SCRIPT:KeepOnFailure");
    add_briefing(&mut peer, "SCRIPT:PeerOnly");

    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(receiver.restore_from_xfer_bytes(&truncated).is_err());
    assert_eq!(history(&receiver), ["SCRIPT:KeepOnFailure"]);
    assert_eq!(history(&peer), ["SCRIPT:PeerOnly"]);

    receiver.restore_from_xfer_bytes(&bytes).unwrap();
    assert_eq!(history(&receiver), ["SCRIPT:SourceOne", "SCRIPT:SourceTwo"]);
    assert_eq!(history(&peer), ["SCRIPT:PeerOnly"]);
}
