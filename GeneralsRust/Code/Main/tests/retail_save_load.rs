//! P0 hq-x1j4u.9.7: retail Lone Eagle save/load round-trip + post-load orders.
//!
//! Test form of `src/bin/retail_save_load_gate.rs`. Fails when retail data,
//! Lone Eagle or the `generals` binary is missing unless
//! `GENERALS_GATES_ALLOW_SKIP=1` (see `common/retail_gate.rs`).

use generals_main::executable_smoke::run_retail_skirmish_save_load;

#[path = "common/retail_gate.rs"]
mod retail_gate;

const GATE: &str = "retail_save_load";

#[test]
fn production_host_saves_and_loads_retail_skirmish() {
    if !retail_gate::require_lone_eagle(GATE) {
        return;
    }
    let r = run_retail_skirmish_save_load(retail_gate::timeout("RETAIL_SAVE_LOAD_TIMEOUT_SECS", 900));
    if !retail_gate::ran(GATE, &r) {
        return;
    }
    let empty_units = r.detail.contains("units=0")
        || r.detail.contains("unit count is 0")
        || r.detail.contains("world_restore_fail:unit count");
    assert!(
        r.executable_host_ok
            && r.reached_ingame
            && r.gameplay_cmd_ok
            && r.save_cmd_ok
            && r.load_cmd_ok
            && !empty_units
            && (r.status == "success" || r.status == "success_forced_exit"),
        "expected save/load round-trip; status={} host_ok={} ingame={} save={} load={} gameplay={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.save_cmd_ok,
        r.load_cmd_ok,
        r.gameplay_cmd_ok,
        r.detail
    );
}
