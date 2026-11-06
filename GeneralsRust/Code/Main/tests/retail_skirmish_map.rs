//! P0 hq-x1j4u.9.2: shell → skirmish → retail non-shell map reaches InGame.
//!
//! Fails when retail data, Lone Eagle or the `generals` binary is missing unless
//! `GENERALS_GATES_ALLOW_SKIP=1` (see `common/retail_gate.rs`).

use generals_main::executable_smoke::{ExecutableSmokeResult, run_load_retail_skirmish_map};

#[path = "common/retail_gate.rs"]
mod retail_gate;

const GATE: &str = "retail_skirmish_map";

#[test]
fn production_host_loads_retail_map_into_skirmish() {
    if !retail_gate::require_lone_eagle(GATE) {
        return;
    }
    let r = run_load_retail_skirmish_map(retail_gate::timeout(
        "RETAIL_SKIRMISH_MAP_TIMEOUT_SECS",
        600,
    ));
    if !retail_gate::ran(GATE, &r) {
        return;
    }
    assert!(
        r.executable_host_ok
            && r.reached_menu
            && r.shell_wnd_ok
            && r.reached_ingame
            && !ExecutableSmokeResult::map_is_shell_for_claim(&r.map_seen),
        "expected retail skirmish map InGame; status={} host_ok={} menu={} shell={} ingame={} map={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_menu,
        r.shell_wnd_ok,
        r.reached_ingame,
        r.map_seen,
        r.detail
    );
    assert!(
        r.status == "success" || r.status == "success_forced_exit",
        "unexpected status {}",
        r.status
    );
}
