use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::campaign_launch_host_bridge::MenuWorldFacts;
use game_client_rust::gui::{mark_host_match_start, tick_main_menu_transitions};

fn world_facts() -> MenuWorldFacts {
    MenuWorldFacts {
        script_engine_available: false,
        chat: None,
        game_mode: 0,
        client_frame: 1,
        logic_frame: 1,
        in_game: false,
        in_shell_game: false,
    }
}

#[test]
fn host_match_shutdown_does_not_enqueue_a_campaign_start() {
    let mut owner = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();

    owner.with_window_event_effects(Some(world_facts()), |effects| {
        mark_host_match_start(effects);
        tick_main_menu_transitions(2, effects);
    });
    peer.with_window_event_effects(Some(world_facts()), |effects| {
        tick_main_menu_transitions(2, effects);
    });

    assert!(
        !owner.has_menu_host_requests(),
        "a host skirmish start must not schedule a campaign reset"
    );
    assert!(
        !peer.has_menu_host_requests(),
        "the other menu remains independent"
    );
}
