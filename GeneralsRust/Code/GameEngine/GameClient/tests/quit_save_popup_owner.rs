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
    popup.push_str(&window(
        "USER",
        "PopupSaveLoad.wnd:SaveLoadMenu",
        (100, 50, 700, 550),
        "SaveLoadMenuSystem",
        false,
    ));
    let names = [
        "MenuButtonFrame",
        "ButtonBack",
        "ButtonSave",
        "ButtonLoad",
        "ButtonDelete",
        "ListboxGames",
        "LoadConfirmParent",
        "ButtonLoadConfirm",
        "ButtonLoadCancel",
        "OverwriteConfirmParent",
        "ButtonOverwriteConfirm",
        "ButtonOverwriteCancel",
        "SaveDescParent",
        "ButtonSaveDescConfirm",
        "ButtonSaveDescCancel",
        "EntryDesc",
        "DeleteConfirmParent",
        "ButtonDeleteConfirm",
        "ButtonDeleteCancel",
    ];
    popup.push_str("CHILD\n");
    for (i, name) in names.iter().enumerate() {
        let (kind, rect, hidden) = if *name == "ButtonBack" {
            ("PUSHBUTTON", (120, 80, 200, 110), false)
        } else if *name == "ListboxGames" {
            ("SCROLLLISTBOX", (220, 90, 400, 180), false)
        } else if *name == "ButtonLoad" {
            ("PUSHBUTTON", (120, 120, 200, 150), false)
        } else {
            ("USER", (500 + i as i32, 400, 501 + i as i32, 401), true)
        };
        popup.push_str(&window(
            kind,
            &format!("PopupSaveLoad.wnd:{name}"),
            rect,
            "[None]",
            hidden,
        ));
        popup.push_str("END\n");
    }
    popup.push_str("ENDALLCHILDREN\nEND\n");
    std::fs::write(menus.join("PopupSaveLoad.wnd"), popup).unwrap();
}
#[test]
fn nested_popup_opens_closes_and_reopens_on_receiving_client() {
    const CHILD: &str = "GENERALS_POPUP_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
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
        let peer =
            GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
        assert!(peer.window_manager_owner().with_manager_ref(|m| {
            m.find_window_by_name("PopupSaveLoad.wnd:SaveLoadMenu")
                .is_none()
        }));
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
