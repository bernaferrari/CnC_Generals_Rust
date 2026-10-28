//! Replay recorder controls are applied to the receiving client's WND tree.
use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::GameWindow;
use game_client_rust::gui::menu_audio::SilentMenuAudio;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::RefCell, rc::Rc};

const PARENT_REPLAY_CONTROL: &str = "ReplayControl.wnd:ParentReplayControl";

fn replay_control_window(client: &mut GameClient) -> Rc<RefCell<GameWindow>> {
    let id = NameKeyGenerator::name_to_key(PARENT_REPLAY_CONTROL) as i32;
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager
                .create_window_with_id(None, 0, 0, 180, 32, id, effects)
                .expect("create receiving replay-controls window")
        })
    })
}

fn deliver_visibility(client: &mut GameClient, hidden: bool) {
    let mut audio = SilentMenuAudio::new();
    client
        .with_load_screen_hooks(&mut audio, |hooks| hooks.replay_controls_visibility(hidden))
        .expect("bind receiving load-screen hook")
        .expect("deliver replay-controls visibility");
}

#[test]
fn load_screen_replay_controls_visibility_stays_with_same_id_client() {
    let mut receiving = GameClient::new().expect("receiving client");
    let mut peer = GameClient::new().expect("peer client");
    let receiving_window = replay_control_window(&mut receiving);
    let peer_window = replay_control_window(&mut peer);
    assert_eq!(
        receiving_window.borrow().get_id(),
        peer_window.borrow().get_id(),
        "both clients intentionally register the same WND ID"
    );

    // GameLogic calls this hook synchronously after Recorder::initControls;
    // the adapter must mutate the manager belonging to the borrowing client.
    deliver_visibility(&mut receiving, true);
    assert!(receiving_window.borrow().is_hidden());
    assert!(!peer_window.borrow().is_hidden());

    deliver_visibility(&mut peer, false);
    assert!(receiving_window.borrow().is_hidden());
    assert!(!peer_window.borrow().is_hidden());
}
