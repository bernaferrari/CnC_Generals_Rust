use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::callbacks::{
    drive_os_wnd_quit_menu_save_load_like_cpp, ensure_live_popup_save_load_layout,
    ensure_live_quit_menu_layout, toggle_quit_menu_receiving,
};
use std::process::Command;

fn window(
    kind: &str,
    name: &str,
    rect: (i32, i32, i32, i32),
    system: &str,
    hidden: bool,
) -> String {
    let (x, y, r, b) = rect;
    format!(
        "WINDOW\n WINDOWTYPE = {kind};\n SCREENRECT = UPPERLEFT: {x} {y}, BOTTOMRIGHT: {r} {b}, CREATIONRESOLUTION: 800 600;\n NAME = \"{name}\";\n STATUS = ENABLED{};\n STYLE = {kind};\n SYSTEMCALLBACK = \"{system}\";\n INPUTCALLBACK = \"[None]\";\n",
        if hidden { "+HIDDEN" } else { "" }
    )
}
fn fixtures(base: &std::path::Path) {
    let menus = base.join("Window/Menus");
    std::fs::create_dir_all(&menus).unwrap();
    let head = "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n LAYOUTINIT = [None];\n LAYOUTUPDATE = [None];\n LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n";
    let mut quit = head.to_string();
    quit.push_str(&window(
        "USER",
        "QuitMenu.wnd:QuitMenuParent",
        (200, 100, 600, 500),
        "QuitMenuSystem",
        false,
    ));
    quit.push_str("CHILD\n");
    quit.push_str(&window(
        "PUSHBUTTON",
        "QuitMenu.wnd:ButtonSaveLoad",
        (240, 130, 440, 170),
        "[None]",
        false,
    ));
    quit.push_str("END\nENDALLCHILDREN\nEND\n");
    std::fs::write(menus.join("QuitMenu.wnd"), quit).unwrap();
    let mut popup = head.replace("LAYOUTINIT = [None]", "LAYOUTINIT = SaveLoadMenuInit");
    popup.push_str(
        &window(
            "USER",
            "PopupSaveLoad.wnd:SaveLoadMenu",
            (100, 50, 700, 550),
            "SaveLoadMenuSystem",
            false,
        )
        .replace("INPUTCALLBACK = \"[None]\"", "INPUTCALLBACK = \"SaveLoadMenuInput\""),
    );
    popup.push_str("CHILD\n");
    popup.push_str(&window(
        "USER", "PopupSaveLoad.wnd:MenuButtonFrame", (110, 70, 210, 240),
        "PassSelectedButtonsToParentSystem", false,
    ));
    popup.push_str("CHILD\n");
    for (name, y) in [
        ("ButtonBack", 80),
        ("ButtonSave", 120),
        ("ButtonLoad", 160),
        ("ButtonDelete", 200),
    ] {
        popup.push_str(&window(
            "PUSHBUTTON", &format!("PopupSaveLoad.wnd:{name}"),
            (120, y, 200, y + 30), "[None]", false,
        ));
        popup.push_str("END\n");
    }
    popup.push_str("ENDALLCHILDREN\nEND\n");
    popup.push_str(&window(
        "SCROLLLISTBOX", "PopupSaveLoad.wnd:ListboxGames",
        (220, 90, 400, 180), "[None]", false,
    ));
    popup.push_str("END\n");
    for (parent, controls) in [
        ("LoadConfirmParent", [("ButtonLoadConfirm", "PUSHBUTTON"), ("ButtonLoadCancel", "PUSHBUTTON"), ("", "")]),
        ("OverwriteConfirmParent", [("ButtonOverwriteConfirm", "PUSHBUTTON"), ("ButtonOverwriteCancel", "PUSHBUTTON"), ("", "")]),
        ("SaveDescParent", [("ButtonSaveDescConfirm", "PUSHBUTTON"), ("ButtonSaveDescCancel", "PUSHBUTTON"), ("EntryDesc", "ENTRYFIELD")]),
        ("DeleteConfirmParent", [("ButtonDeleteConfirm", "PUSHBUTTON"), ("ButtonDeleteCancel", "PUSHBUTTON"), ("", "")]),
    ] {
        popup.push_str(&window(
            "USER", &format!("PopupSaveLoad.wnd:{parent}"), (410, 200, 690, 360),
            "PassSelectedButtonsToParentSystem", true,
        ));
        popup.push_str("CHILD\n");
        for (index, (name, kind)) in controls.into_iter().enumerate() {
            if !name.is_empty() {
                let y = 210 + index as i32 * 40;
                popup.push_str(&window(
                    kind, &format!("PopupSaveLoad.wnd:{name}"), (430, y, 570, y + 30),
                    "[None]", false,
                ));
                popup.push_str("END\n");
            }
        }
        popup.push_str("ENDALLCHILDREN\nEND\n");
    }
    popup.push_str("ENDALLCHILDREN\nEND\n");
    std::fs::write(menus.join("PopupSaveLoad.wnd"), popup).unwrap();
}

fn click(client: &mut GameClient, name: &str) -> bool {
    client.with_window_event_effects(None, |effects| {
        game_client_rust::gui::dispatch_os_click_named_window(
            &format!("PopupSaveLoad.wnd:{name}"),
            effects,
        )
    })
}

fn enabled(client: &GameClient, name: &str) -> bool {
    client.window_manager_owner().with_manager_ref(|manager| {
        manager
            .find_window_by_name(&format!("PopupSaveLoad.wnd:{name}"))
            .unwrap()
            .borrow()
            .is_enabled()
    })
}

fn hidden(client: &GameClient, name: &str) -> bool {
    client.window_manager_owner().with_manager_ref(|manager| {
        manager
            .find_window_by_name(&format!("PopupSaveLoad.wnd:{name}"))
            .unwrap()
            .borrow()
            .is_hidden()
    })
}

fn open_popup(client: &mut GameClient) {
    client.with_window_event_effects(None, |effects| {
        assert!(drive_os_wnd_quit_menu_save_load_like_cpp(effects));
        assert!(effects.with_manager(|manager, effects| {
            ensure_live_popup_save_load_layout(manager, effects)
        }));
    });
}
#[test]
fn nested_popup_opens_closes_and_reopens_on_receiving_client() {
    const CHILD: &str = "GENERALS_POPUP_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let _services = game_engine::common::common_services::CommonServicesScope::new();
        let world = gamelogic::system::engine_stores::new_world_services();
        let peer_world = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&world, || {
        let mut client =
            GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
        client.set_in_game_ui_subsystem(InGameUISubsystem::default());
        client.with_window_event_effects(None, |effects| {
            toggle_quit_menu_receiving(false, effects, None);
            assert!(effects.with_manager(|m, e| ensure_live_quit_menu_layout(m, e)));
            assert!(!effects.with_manager(|m, _| {
                game_client_rust::gui::callbacks::live_popup_save_load_is_visible(m)
            }));
            assert!(drive_os_wnd_quit_menu_save_load_like_cpp(effects));
            assert!(effects.with_manager(|m, e| ensure_live_popup_save_load_layout(m, e)));
            assert!(effects.with_manager(|m, _| {
                game_client_rust::gui::callbacks::live_popup_save_load_is_visible(m)
            }));
        });
        client.publish_popup_save_load_entries(vec![
            game_client_rust::gui::callbacks::PopupSaveLoadEntry {
                filename: "new_save".into(),
                description: "new_save".into(),
                ..Default::default()
            },
        ]);
        client.with_window_event_effects(None, |effects| {
            assert!(game_client_rust::gui::dispatch_os_click_named_window(
                "PopupSaveLoad.wnd:ButtonBack",
                effects
            ));
            assert!(!effects.with_manager(|m, _| {
                game_client_rust::gui::callbacks::live_popup_save_load_is_visible(m)
            }));
            assert!(drive_os_wnd_quit_menu_save_load_like_cpp(effects));
            assert!(effects.with_manager(|m, e| ensure_live_popup_save_load_layout(m, e)));
            assert!(
                game_client_rust::gui::callbacks::select_live_popup_save_game_named_like_cpp(
                    "new_save", effects
                )
            );
            assert!(effects.with_manager(|m, _| {
                m.find_window_by_name("PopupSaveLoad.wnd:ButtonLoad")
                    .is_some_and(|button| button.borrow().is_enabled())
            }));
        });
        let mut peer = gamelogic::system::engine_stores::with_world_services(&peer_world, || {
            let mut peer =
                GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
            assert!(peer.window_manager_owner().with_manager_ref(|m| {
                m.find_window_by_name("PopupSaveLoad.wnd:SaveLoadMenu")
                    .is_none()
            }));
            peer.set_in_game_ui_subsystem(InGameUISubsystem::default());
            peer.publish_popup_save_load_entries(Vec::new());
            peer.with_window_event_effects(None, |effects| {
                toggle_quit_menu_receiving(false, effects, None);
                assert!(effects.with_manager(|m, e| ensure_live_quit_menu_layout(m, e)));
            });
            open_popup(&mut peer);
            peer
        });

        // Reopen to select the New Save Game row, then use real WND buttons.
        assert!(click(&mut client, "ButtonBack"));
        open_popup(&mut client);
        assert!(click(&mut client, "ButtonSave"));
        assert!(!hidden(&client, "SaveDescParent"));
        assert!(!enabled(&client, "ListboxGames"));
        assert!(!enabled(&client, "MenuButtonFrame"));
        assert!(!click(&mut client, "ButtonBack"));
        gamelogic::system::engine_stores::with_world_services(&peer_world, || {
            assert!(enabled(&peer, "ListboxGames"));
            assert!(enabled(&peer, "MenuButtonFrame"));
            assert!(hidden(&peer, "SaveDescParent"));
            assert!(peer.take_popup_save_load_requests().is_empty());
        });

        // Escape forwards Back even while the main frame is disabled.
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager.process_key_event(0x1b, 0x01, effects);
            });
        });
        assert!(hidden(&client, "SaveDescParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "MenuButtonFrame"));
        open_popup(&mut client);
        assert!(hidden(&client, "SaveDescParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "ButtonSave"));
        assert!(client.take_popup_save_load_requests().is_empty());

        assert!(click(&mut client, "ButtonSave"));
        assert!(click(&mut client, "ButtonSaveDescCancel"));
        assert!(hidden(&client, "SaveDescParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "ButtonSave"));
        assert!(client.take_popup_save_load_requests().is_empty());

        assert!(click(&mut client, "ButtonSave"));
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, _| {
                manager
                    .find_window_by_name("PopupSaveLoad.wnd:EntryDesc")
                    .unwrap()
                    .borrow_mut()
                    .text_entry_mut()
                    .unwrap()
                    .set_text("Popup regression save");
            });
        });
        assert!(click(&mut client, "ButtonSaveDescConfirm"));
        assert!(hidden(&client, "SaveDescParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "MenuButtonFrame"));
        // Reinitialization must not discard a save awaiting host processing.
        open_popup(&mut client);
        assert!(hidden(&client, "SaveDescParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "ButtonSave"));
        let requests = client.take_popup_save_load_requests();
        assert_eq!(requests.len(), 1);
        assert!(matches!(
            &requests[0].request,
            game_client_rust::gui::callbacks::PopupSaveLoadRequest::Save {
                filename, description, ..
            } if filename.is_empty() && description == "Popup regression save"
        ));
        client.with_window_event_effects(None, |effects| {
            assert!(game_client_rust::gui::callbacks::select_live_popup_save_game_named_like_cpp(
                "new_save", effects
            ));
        });
        assert!(enabled(&client, "ButtonLoad"));
        assert!(click(&mut client, "ButtonSave"));
        assert!(!hidden(&client, "OverwriteConfirmParent"));
        assert!(!enabled(&client, "ListboxGames"));
        assert!(!enabled(&client, "MenuButtonFrame"));
        assert!(click(&mut client, "ButtonOverwriteCancel"));
        assert!(hidden(&client, "OverwriteConfirmParent"));
        assert!(enabled(&client, "ListboxGames"));
        assert!(enabled(&client, "ButtonLoad"));
        assert!(click(&mut client, "ButtonSave"));
        assert!(click(&mut client, "ButtonOverwriteConfirm"));
        open_popup(&mut client);
        assert!(hidden(&client, "OverwriteConfirmParent"));
        assert!(enabled(&client, "ListboxGames"));
        let requests = client.take_popup_save_load_requests();
        assert_eq!(requests.len(), 1);
        assert!(matches!(
            &requests[0].request,
            game_client_rust::gui::callbacks::PopupSaveLoadRequest::Save {
                filename, description, ..
            } if filename == "new_save" && description == "new_save"
        ));
        gamelogic::system::engine_stores::with_world_services(&peer_world, || {
            assert!(peer.take_popup_save_load_requests().is_empty());
            drop(peer);
        });
        });
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let assets = tmp.path().join("assets");
    let cwd = tmp.path().join("cwd");
    fixtures(&assets);
    std::fs::create_dir_all(&cwd).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("nested_popup_opens_closes_and_reopens_on_receiving_client")
        .arg("--nocapture")
        .current_dir(cwd)
        .env(CHILD, "1")
        .env("GENERALS_ASSETS_DIR", assets)
        .status()
        .unwrap();
    assert!(status.success());
}
