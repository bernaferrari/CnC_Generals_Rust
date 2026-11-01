use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::shell::main_menu::soft_reveal_main_menu_for_host_inject;

#[test]
fn first_input_reveals_only_the_receiving_clients_main_dropdown() {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("MainMenu.wnd");
    std::fs::write(
        &path,
        r#"WINDOW
WINDOWTYPE=USER;
NAME="MainMenu.wnd:Root";
SCREENRECT=0 0 100 100;
CHILD
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
    let setup = |client: &mut GameClient| {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows(path.to_str().unwrap(), effects)
                    .unwrap();
                let dropdown = manager
                    .find_window_by_name("MainMenu.wnd:MapBorder2")
                    .unwrap();
                let _ = dropdown.borrow_mut().hide(true, manager);
                dropdown
            })
        })
    };
    let receiving = setup(&mut client);
    let other = setup(&mut peer);
    client.with_window_event_effects(None, |effects| {
        assert!(soft_reveal_main_menu_for_host_inject(effects));
    });
    assert!(
        !receiving.borrow().is_hidden(),
        "the receiving dropdown must become visible"
    );
    assert!(
        other.borrow().is_hidden(),
        "the peer dropdown must remain hidden"
    );
}
