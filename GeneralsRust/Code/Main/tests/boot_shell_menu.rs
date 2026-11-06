//! P0 hq-x1j4u.9.1: production host boots to live MainMenu.wnd with retail assets.
//!
//! Fails when retail data or the `generals` binary is missing unless
//! `GENERALS_GATES_ALLOW_SKIP=1` (see `common/retail_gate.rs`).

use generals_main::executable_smoke::run_boot_to_shell_menu;

#[path = "common/retail_gate.rs"]
mod retail_gate;

const GATE: &str = "boot_shell_menu";

#[test]
fn production_host_boots_to_main_menu_wnd() {
    let Some(options) = retail_gate::require_retail(GATE) else {
        return;
    };
    let r = run_boot_to_shell_menu(retail_gate::timeout("BOOT_SHELL_MENU_TIMEOUT_SECS", 180), &options);
    if !retail_gate::ran(GATE, &r) {
        return;
    }
    assert!(
        r.executable_host_ok && r.reached_menu && r.shell_wnd_ok,
        "expected boot to MainMenu.wnd; status={} host_ok={} menu={} shell={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_menu,
        r.shell_wnd_ok,
        r.detail
    );
    assert!(
        r.status == "success" || r.status == "success_forced_exit",
        "unexpected status {}",
        r.status
    );
}
