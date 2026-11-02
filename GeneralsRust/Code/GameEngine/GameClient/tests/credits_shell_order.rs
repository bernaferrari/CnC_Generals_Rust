use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::game_window::WindowMessage;

#[test]
fn credits_reveals_main_only_after_the_shell_push_boundary() {
    use game_client_rust::gui::campaign_launch_host_bridge::MenuWorldFacts;
    use game_client_rust::gui::shell::main_menu::{
        simulate_main_menu_options_button_gadget_selected, tick_main_menu_transitions,
    };
    use game_client_rust::system::SubsystemInterface;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    let fixture = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    struct RestoreDir(std::path::PathBuf);
    impl Drop for RestoreDir {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }
    let _restore = RestoreDir(original_dir);
    std::env::set_current_dir(fixture.path()).unwrap();
    std::fs::create_dir_all("windows_game/extracted_big_files_v2/WindowZH/Window/Menus").unwrap();
    std::fs::write(
        "windows_game/extracted_big_files_v2/WindowZH/Window/Menus/CreditsMenu.wnd",
        "WINDOW\nWINDOWTYPE=USER;\nNAME=\"CreditsMenu.wnd:Root\";\nSCREENRECT=0 0 100 100;\nEND\n",
    )
    .unwrap();
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
ENDALLCHILDREN
END
"#,
    )
    .unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    client
        .with_receiving_shell(None, |shell, _| shell.init().unwrap())
        .unwrap();
    let setup = |client: &mut GameClient| {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows(path.to_str().unwrap(), effects)
                    .unwrap();
                let root = manager.find_window_by_name("MainMenu.wnd:Root").unwrap();
                let main = manager
                    .find_window_by_name("MainMenu.wnd:MapBorder2")
                    .unwrap();
                let _ = main.borrow_mut().hide(true, manager);
                (root, main)
            })
        })
    };
    let (root, main) = setup(&mut client);
    let (_, peer_main) = setup(&mut peer);
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
            let _ = main.borrow_mut().hide(true, manager);
            manager.send_system_message(
                &root,
                WindowMessage::GadgetSelected,
                NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonCredits") as usize,
                0,
                effects,
            );
            assert!(
                main.borrow().is_hidden(),
                "shell push must run before the main dropdown is revealed"
            );
        });
    });
    assert!(
        !main.borrow().is_hidden(),
        "the completed continuation reveals the main dropdown"
    );
    client
        .with_receiving_shell(None, |shell, _| {
            assert_eq!(shell.top_filename(), Some("Menus/CreditsMenu.wnd"));
        })
        .unwrap();
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, _| {
            assert!(
                manager
                    .find_window_by_name("CreditsMenu.wnd:Root")
                    .is_some(),
                "Credits layout must be initialized"
            );
        });
    });
    assert!(
        peer_main.borrow().is_hidden(),
        "the peer client remains untouched"
    );

    // Exercise the real MainMenuShutdown callback when Credits is pushed over
    // a populated shell stack. Shell::push leaves Credits pending until the
    // outgoing layout reports shutdown completion.
    let asset_dir = "windows_game/extracted_big_files_v2/WindowZH/Window/Menus";
    let main_script = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        format!("{asset_dir}/MainMenu.wnd"),
        format!("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n LAYOUTINIT = [None];\n LAYOUTUPDATE = [None];\n LAYOUTSHUTDOWN = [MainMenuShutdown];\nENDLAYOUTBLOCK\n{main_script}"),
    ).unwrap();
    let mut populated = GameClient::new().unwrap();
    populated
        .with_receiving_shell(Some(facts), |shell, effects| {
            shell.init().unwrap();
            shell.push("Menus/MainMenu.wnd", false, effects).unwrap();
            assert_eq!(shell.top_filename(), Some("Menus/MainMenu.wnd"));
        })
        .unwrap();
    let (root, main) = populated.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, _| {
            (
                manager.find_window_by_name("MainMenu.wnd:Root").unwrap(),
                manager
                    .find_window_by_name("MainMenu.wnd:MapBorder2")
                    .unwrap(),
            )
        })
    });
    populated.with_window_event_effects(Some(facts), |effects| {
        assert!(simulate_main_menu_options_button_gadget_selected(effects));
        tick_main_menu_transitions(1, effects);
        effects.with_manager(|manager, effects| {
            let _ = main.borrow_mut().hide(true, manager);
            manager.send_system_message(
                &root,
                WindowMessage::GadgetSelected,
                NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonCredits") as usize,
                0,
                effects,
            );
            assert!(
                main.borrow().is_hidden(),
                "outgoing callback has not yet released its window loan"
            );
        });
    });
    assert!(
        !main.borrow().is_hidden(),
        "Credits continuation must reveal the original menu"
    );
    populated
        .with_receiving_shell(Some(facts), |shell, _| {
            assert_eq!(
                shell.top_filename(),
                Some("Menus/MainMenu.wnd"),
                "push waits for MainMenuShutdown"
            );
        })
        .unwrap();
    populated.with_window_event_effects(Some(facts), |effects| {
        tick_main_menu_transitions(2, effects);
    });
    populated
        .with_receiving_shell(Some(facts), |shell, _| {
            assert_eq!(
                shell.top_filename(),
                Some("Menus/CreditsMenu.wnd"),
                "MainMenuShutdown must lead to the pending Credits push"
            );
        })
        .unwrap();
    assert!(
        peer_main.borrow().is_hidden(),
        "the populated-shell path must leave its peer untouched"
    );
}
