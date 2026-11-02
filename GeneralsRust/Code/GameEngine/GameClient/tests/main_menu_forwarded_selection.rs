use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::game_window::WindowMessage;

#[test]
fn nested_parent_selection_updates_the_receiving_menu_before_returning() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("MainMenu.wnd");
    std::fs::write(
        &path,
        r#"WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:Root";
SYSTEMCALLBACK=MainMenuSystem;
SCREENRECT=0 0 100 100;
CHILD
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder1";
SYSTEMCALLBACK=PassMessagesToParentSystem;
SCREENRECT=0 0 100 100;
CHILD
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:Selected";
SYSTEMCALLBACK=PassSelectedButtonsToParentSystem;
SCREENRECT=0 0 100 100;
END
ENDALLCHILDREN
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder";
SCREENRECT=0 0 100 100;
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder2";
SCREENRECT=0 0 100 100;
END
ENDALLCHILDREN
END
"#,
    )
    .unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let peer_dropdown = peer.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager
                .create_layout_with_windows(path.to_str().unwrap(), effects)
                .unwrap();
            manager
                .find_window_by_name("MainMenu.wnd:MapBorder1")
                .unwrap()
        })
    });
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager
                .create_layout_with_windows(path.to_str().unwrap(), effects)
                .unwrap();
            let selected = manager
                .find_window_by_name("MainMenu.wnd:Selected")
                .unwrap();
            let single = manager
                .find_window_by_name("MainMenu.wnd:MapBorder")
                .unwrap();
            let multiplayer = manager
                .find_window_by_name("MainMenu.wnd:MapBorder1")
                .unwrap();
            let main = manager
                .find_window_by_name("MainMenu.wnd:MapBorder2")
                .unwrap();
            let _ = single.borrow_mut().hide(true, manager);
            let _ = multiplayer.borrow_mut().hide(true, manager);
            // Before MainMenuInit, the default Single Player control id is zero.
            // Forwarding this event exercises the same three borrowed windows.
            let result = manager.send_system_message(
                &selected,
                WindowMessage::GadgetSelected,
                0,
                0,
                effects,
            );
            assert!(result.is_handled());
            assert!(!single.borrow().is_hidden());
            assert!(multiplayer.borrow().is_hidden());
            assert!(
                !main.borrow().is_hidden(),
                "C++ starts the transition without hiding the main dropdown"
            );
        });
    });
    assert!(
        !peer_dropdown.borrow().is_hidden(),
        "the other client retains its own window state"
    );
}

#[test]
fn load_replay_transition_preserves_other_receiving_dropdowns() {
    use game_client_rust::gui::shell::main_menu::{
        simulate_main_menu_options_button_gadget_selected, tick_main_menu_transitions,
    };
    use game_engine::common::name_key_generator::NameKeyGenerator;

    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("MainMenu.wnd");
    std::fs::write(
        &path,
        r#"WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:Root";
SYSTEMCALLBACK=MainMenuSystem;
SCREENRECT=0 0 100 100;
CHILD
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder2";
SCREENRECT=0 0 100 100;
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder3";
SCREENRECT=0 0 100 100;
END
ENDALLCHILDREN
END
"#,
    )
    .unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let setup = |client: &mut GameClient| {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows(path.to_str().unwrap(), effects)
                    .unwrap();
                let main = manager
                    .find_window_by_name("MainMenu.wnd:MapBorder2")
                    .unwrap();
                let load = manager
                    .find_window_by_name("MainMenu.wnd:MapBorder3")
                    .unwrap();
                let root = manager.find_window_by_name("MainMenu.wnd:Root").unwrap();
                let _ = load.borrow_mut().hide(true, manager);
                (root, main, load)
            })
        })
    };
    let (root, main, load) = setup(&mut client);
    let (_, peer_main, peer_load) = setup(&mut peer);
    let facts = game_client_rust::gui::campaign_launch_host_bridge::MenuWorldFacts {
        script_engine_available: false,
        chat: None,
        game_mode: 0,
        client_frame: 1,
        logic_frame: 1,
        in_game: false,
        in_shell_game: false,
    };
    client.with_window_event_effects(Some(facts), |effects| {
        assert!(simulate_main_menu_options_button_gadget_selected(effects));
        tick_main_menu_transitions(1, effects);
        effects.with_manager(|manager, effects| {
            let _ = main.borrow_mut().hide(false, manager);
            let _ = load.borrow_mut().hide(true, manager);
            manager.send_system_message(
                &root,
                WindowMessage::GadgetSelected,
                NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonLoadReplay") as usize,
                0,
                effects,
            );
        });
    });
    assert!(!load.borrow().is_hidden(), "destination must be shown");
    assert!(
        !main.borrow().is_hidden(),
        "C++ leaves the outgoing dropdown visible during the transition"
    );
    assert!(!peer_main.borrow().is_hidden());
    assert!(peer_load.borrow().is_hidden());
}

#[test]
fn back_and_multiplayer_transitions_only_unhide_the_destination() {
    use game_client_rust::gui::campaign_launch_host_bridge::MenuWorldFacts;
    use game_client_rust::gui::shell::main_menu::{
        simulate_main_menu_options_button_gadget_selected, tick_main_menu_transitions,
    };
    use game_engine::common::name_key_generator::NameKeyGenerator;

    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("MainMenu.wnd");
    std::fs::write(
        &path,
        r#"WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:Root";
SYSTEMCALLBACK=MainMenuSystem;
SCREENRECT=0 0 100 100;
CHILD
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder1";
SCREENRECT=0 0 100 100;
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder";
SCREENRECT=0 0 100 100;
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder3";
SCREENRECT=0 0 100 100;
END
WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:MapBorder2";
SCREENRECT=0 0 100 100;
END
ENDALLCHILDREN
END
"#,
    )
    .unwrap();
    for (button, destination, outgoing) in [
        ("ButtonMultiplayer", "MapBorder1", "MapBorder2"),
        ("ButtonSingleBack", "MapBorder2", "MapBorder1"),
        ("ButtonMultiBack", "MapBorder2", "MapBorder1"),
        ("ButtonLoadReplayBack", "MapBorder2", "MapBorder1"),
        ("ButtonLoadGame", "MapBorder3", "MapBorder2"),
        ("ButtonReplay", "MapBorder3", "MapBorder2"),
        ("ButtonSkirmish", "MapBorder", "MapBorder2"),
    ] {
        let mut client = GameClient::new().unwrap();
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows(path.to_str().unwrap(), effects)
                    .unwrap();
            })
        });
        let facts = MenuWorldFacts {
            script_engine_available: false,
            chat: None,
            game_mode: 0,
            client_frame: 1,
            logic_frame: 1,
            in_game: false,
            in_shell_game: false,
        };
        client.with_window_event_effects(Some(facts), |effects| {
            assert!(simulate_main_menu_options_button_gadget_selected(effects));
            tick_main_menu_transitions(1, effects);
            effects.with_manager(|manager, effects| {
                let root = manager.find_window_by_name("MainMenu.wnd:Root").unwrap();
                let dest = manager
                    .find_window_by_name(&format!("MainMenu.wnd:{destination}"))
                    .unwrap();
                let old = manager
                    .find_window_by_name(&format!("MainMenu.wnd:{outgoing}"))
                    .unwrap();
                let _ = dest.borrow_mut().hide(true, manager);
                let _ = old.borrow_mut().hide(false, manager);
                manager.send_system_message(
                    &root,
                    WindowMessage::GadgetSelected,
                    NameKeyGenerator::name_to_key(&format!("MainMenu.wnd:{button}")) as usize,
                    0,
                    effects,
                );
                assert!(
                    !dest.borrow().is_hidden(),
                    "{button} must reveal {destination}"
                );
                assert!(
                    !old.borrow().is_hidden(),
                    "{button} must retain {outgoing} for its transition"
                );
            });
        });
    }
}
