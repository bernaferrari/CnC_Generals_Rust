//! P0 hq-x1j4u.9.5: AI activity on retail Lone Eagle while human issues move orders.
//!
//! Fails when retail data, Lone Eagle or the `generals` binary is missing unless
//! `GENERALS_GATES_ALLOW_SKIP=1` (see `common/retail_gate.rs`).

use generals_main::executable_smoke::run_ai_opponent_skirmish;

#[path = "common/retail_gate.rs"]
mod retail_gate;

const GATE: &str = "ai_retail_skirmish";

#[test]
fn production_host_ai_runs_with_human_orders_on_retail_map() {
    let Some(options) = retail_gate::require_lone_eagle(GATE) else {
        return;
    };
    let r = run_ai_opponent_skirmish(retail_gate::timeout("AI_RETAIL_SKIRMISH_TIMEOUT_SECS", 900), &options);
    if !retail_gate::ran(GATE, &r) {
        return;
    }
    assert!(
        r.executable_host_ok
            && r.reached_ingame
            && r.gameplay_cmd_ok
            && (r.status == "success" || r.status == "success_forced_exit"),
        "expected AI activity + human move; status={} host_ok={} ingame={} gameplay={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.detail
    );
}
