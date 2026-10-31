use generals_main::gameplay_layout::{
    control_bar_layout_honesty_with_owner, control_bar_parent_is_live_with_owner,
};
use std::process::Command;

#[test]
fn control_bar_uses_receiving_window_resolver_when_cwd_has_no_assets() {
    const CHILD: &str = "GENERALS_CONTROL_BAR_OWNER_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut client = game_client::game_client::GameClient::new().unwrap();
        let owner = client.window_manager_owner();
        let result = client.with_window_event_effects(None, |effects| {
            control_bar_layout_honesty_with_owner(true, effects)
        });
        assert!(
            result.window_loaded,
            "configured asset root should load the bar: {result:?}"
        );
        assert!(control_bar_parent_is_live_with_owner(&owner));
        let peer = game_client::game_client::GameClient::new().unwrap();
        assert!(!control_bar_parent_is_live_with_owner(
            &peer.window_manager_owner()
        ));
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let assets = temp.path().join("assets");
    let cwd = temp.path().join("cwd");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(
        assets.join("ControlBar.wnd"),
        r#"FILE_VERSION = 2;
STARTLAYOUTBLOCK
  LAYOUTINIT = [None];
  LAYOUTUPDATE = [None];
  LAYOUTSHUTDOWN = [None];
ENDLAYOUTBLOCK
WINDOW
  WINDOWTYPE = USER;
  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;
  NAME = "ControlBar.wnd:ControlBarParent";
  STATUS = ENABLED;
  STYLE = USER;
END
"#,
    )
    .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("control_bar_uses_receiving_window_resolver_when_cwd_has_no_assets")
        .arg("--nocapture")
        .current_dir(cwd)
        .env(CHILD, "1")
        .env("GENERALS_ASSETS_DIR", assets)
        .status()
        .unwrap();
    assert!(status.success());
}
