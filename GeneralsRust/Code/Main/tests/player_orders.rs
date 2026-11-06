//! P0 hq-x1j4u.9.3: select/move/attack on a loaded retail skirmish map.
//!
//! Fails when retail data, Lone Eagle or the `generals` binary is missing unless
//! `GENERALS_GATES_ALLOW_SKIP=1` (see `common/retail_gate.rs`).

use generals_main::executable_smoke::run_issue_player_orders;

#[path = "common/retail_gate.rs"]
mod retail_gate;

const GATE: &str = "player_orders";

#[test]
fn production_host_issues_select_move_attack_orders() {
    let Some(options) = retail_gate::require_lone_eagle(GATE) else {
        return;
    };
    let r = run_issue_player_orders(retail_gate::timeout("PLAYER_ORDERS_TIMEOUT_SECS", 720), &options);
    if !retail_gate::ran(GATE, &r) {
        return;
    }
    assert!(
        r.executable_host_ok
            && r.reached_ingame
            && r.gameplay_cmd_ok
            && (r.status == "success" || r.status == "success_forced_exit"),
        "expected select/move/attack; status={} host_ok={} ingame={} gameplay={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.detail
    );
}
