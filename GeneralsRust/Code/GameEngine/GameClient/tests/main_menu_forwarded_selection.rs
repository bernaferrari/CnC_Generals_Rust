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
