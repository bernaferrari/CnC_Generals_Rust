//! Public shell update ownership regression with an authored WND fixture.
use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::campaign_launch_host_bridge::{MenuWorldFacts, ShellMapSettings};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

struct RestoreCurrentDir(PathBuf);

impl Drop for RestoreCurrentDir {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).expect("restore test working directory");
    }
}

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

fn prepare_client() -> GameClient {
    let mut client = GameClient::new().expect("construct client");
    client.configure_shell_map(ShellMapSettings {
        initial_file: String::new(),
        enabled: true,
        map: "unused-shell-map.map".into(),
    });
    client
        .window_manager_owner()
        .shell_handle()
        .init()
        .expect("initialize shell");

    // MainMenuInit's public path installs the background layout; then mark the
    // shell-map flag as active while retaining that background for Shell::update.
    client
        .with_receiving_shell(Some(world_facts()), |shell, effects| {
            shell.show_shell_map(false, effects);
        })
        .expect("receiving shell available");
    client
        .with_receiving_shell(Some(world_facts()), |shell, effects| {
            shell.show_shell_map(true, effects);
        })
        .expect("receiving shell available");
    client
}

#[test]
fn shell_update_destroys_only_the_enabled_receiving_clients_background() {
    let original_dir = std::env::current_dir().expect("current directory");
    let _restore_dir = RestoreCurrentDir(original_dir);
    let fixture = tempfile::tempdir().expect("temporary WND asset root");
    let menus = fixture.path().join("Window/Menus");
    std::fs::create_dir_all(&menus).expect("create WND asset directory");
    std::fs::write(
        menus.join("BlankWindow.wnd"),
        "WINDOW\nWINDOWTYPE=USER;\nNAME=\"ShellMapBackgroundFixture\";\nSTATUS=ENABLED+TABSTOP;\nSCREENRECT=0 0 100 100;\nEND\n",
    )
    .expect("write valid background WND");
    std::env::set_current_dir(fixture.path()).expect("use isolated WND fixture");

    let global = game_engine::common::ini::ini_game_data::ensure_global_data();
    {
        let mut definitions = global.write();
        definitions.initial_file.clear();
        definitions.shell_map_on = true;
    }

    let mut enabled = prepare_client();
    let mut peer = prepare_client();
    let enabled_windows = enabled
        .window_manager_owner()
        .with_manager_ref(|manager| manager.window_count());
    let peer_windows = peer
        .window_manager_owner()
        .with_manager_ref(|manager| manager.window_count());
    assert_eq!(enabled_windows, 1, "fixture background was installed");
    assert_eq!(peer_windows, 1, "peer background was installed");

    // The receiving peer changes its admitted policy after the background is
    // installed. Both shell-map flags remain true, while GlobalData remains true.
    peer.configure_shell_map(ShellMapSettings {
        initial_file: String::new(),
        enabled: false,
        map: "unused-shell-map.map".into(),
    });

    thread::sleep(Duration::from_millis(40));
    enabled
        .with_receiving_shell(Some(world_facts()), |shell, effects| {
            shell
                .update_receiving(effects)
                .expect("update enabled shell");
        })
        .expect("receiving shell available");
    peer.with_receiving_shell(Some(world_facts()), |shell, effects| {
        shell
            .update_receiving(effects)
            .expect("update disabled shell");
    })
    .expect("receiving shell available");

    assert_eq!(
        enabled
            .window_manager_owner()
            .with_manager_ref(|manager| manager.window_count()),
        0,
        "enabled receiving shell destroys its background window"
    );
    assert_eq!(
        peer.window_manager_owner()
            .with_manager_ref(|manager| manager.window_count()),
        1,
        "disabled receiving shell retains its background despite enabled process GlobalData"
    );
}
