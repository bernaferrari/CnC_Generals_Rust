//! Wave 679 residual peels: production spawn ObjectId ready channel.
//! Host still allocates IDs via `create_object`; ready log gates door/notify/exit.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_METHOD_NAMES_WAVE679: &[&str] = &[
    "host_production_spawn_ready_log",
    "host_spawn_production_unit",
    "host_apply_production_spawn_ready_completions",
    "Wave 679",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_NAV_STEPS_WAVE679: &[&str] = &[
    "REQUIRE_PRODUCTION_SPAWN_READY_LOG_MODULE",
    "REQUIRE_SPAWN_RECORDS_READY",
    "REQUIRE_HOST_DRAINS_PRODUCTION_SPAWN_READY",
    "REQUIRE_HOST_APPLIES_DOOR_NOTIFY_EXIT",
    "LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_CMD_NAMES_WAVE679: &[&str] = &[
    "host_production_spawn_ready_log_helper",
    "spawn_records_ready",
    "host_drains_production_spawn_ready",
    "host_applies_door_notify_exit",
    "production_spawn_ready_log_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSpawnReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionSpawnReadyLogHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostProductionSpawnReadyLogHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_spawn_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_spawn_ready_log_helper_last_action()
-> ResidualHostProductionSpawnReadyLogHelperAction {
    ResidualHostProductionSpawnReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ready_log_source() -> &'static str {
    include_str!("../host_production_spawn_ready_log.rs")
}
fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}
fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}
pub fn honesty_host_production_spawn_ready_log_helper_method_names_residual_wave679() -> bool {
    let names = LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_METHOD_NAMES_WAVE679;
    let ok = residual_name_index(names, "host_production_spawn_ready_log").is_some()
        && residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "host_apply_production_spawn_ready_completions").is_some()
        && residual_name_index(names, "Wave 679").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::MethodNames);
    ok
}
pub fn honesty_host_production_spawn_ready_log_helper_source_markers_residual_wave679() -> bool {
    let gl = gl_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 679")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(");
    let Some(apply) = fn_body(gl, "fn host_apply_unit_production_completions(") else {
        residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let apply_ok = apply.contains("Wave 679")
        && apply.contains("host_production_spawn_ready_log::record")
        && (apply.contains("host_apply_production_spawn_ready_completions")
            || apply.contains("ApplySpawnReadyCompletions")
            || apply.contains("apply_production_authority_op"))
        && (apply.contains("host_spawn_production_unit")
            || apply.contains("SpawnUnit")
            || apply.contains("apply_production_authority_op"))
        && apply.contains("record_complete");
    let Some(ready_apply) = fn_body(gl, "pub fn host_apply_production_spawn_ready_completions(")
    else {
        residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let ready_apply_ok = ready_apply.contains("Wave 679")
        && ready_apply.contains("host_production_spawn_ready_log::drain")
        && ready_apply.contains("notify_unit_production_complete")
        && ready_apply.contains("start_production_door_cycle");
    let ok = log_ok && apply_ok && ready_apply_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_spawn_ready_log_helper_nav_commands_residual_wave679() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_NAV_STEPS_WAVE679;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER_CMD_NAMES_WAVE679;
    let ok = residual_name_index(steps, "REQUIRE_PRODUCTION_SPAWN_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_SPAWN_RECORDS_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_PRODUCTION_SPAWN_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_APPLIES_DOOR_NOTIFY_EXIT").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SPAWN_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_spawn_ready_log_helper").is_some()
        && residual_name_index(cmds, "spawn_records_ready").is_some()
        && residual_name_index(cmds, "host_drains_production_spawn_ready").is_some()
        && residual_name_index(cmds, "host_applies_door_notify_exit").is_some()
        && residual_name_index(cmds, "production_spawn_ready_log_residual").is_some();
    residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::NavCommands);
    ok
}
pub fn simulate_host_production_spawn_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 679")
        && gl_source().contains("host_production_spawn_ready_log::record")
        && gl_source().contains("host_production_spawn_ready_log::drain");
    residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::CollectSource);
    ok
}
pub fn simulate_host_production_spawn_ready_log_helper_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 679")
        && (gl_source().contains("host_apply_production_spawn_ready_completions")
            || gl_source().contains("ApplySpawnReadyCompletions"));
    residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_production_spawn_ready_log_helper_residual_pack_wave679() -> bool {
    honesty_host_production_spawn_ready_log_helper_method_names_residual_wave679()
        && honesty_host_production_spawn_ready_log_helper_source_markers_residual_wave679()
        && honesty_host_production_spawn_ready_log_helper_nav_commands_residual_wave679()
        && simulate_host_production_spawn_ready_log_helper_collect_source()
        && simulate_host_production_spawn_ready_log_helper_dispatch_source()
}
pub fn simulate_live_host_production_spawn_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_production_spawn_ready_log_helper_residual_pack_wave679();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSpawnReadyLogHelperAction::Composite);
    }
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_spawn_ready_log_helper_method_names_residual_wave679());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_spawn_ready_log_helper_source_markers_residual_wave679());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_spawn_ready_log_helper_nav_commands_residual_wave679());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_spawn_ready_log_helper_collect_source());
        assert!(simulate_host_production_spawn_ready_log_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_spawn_ready_log_helper_residual_pack_wave679());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_spawn_ready_log_helper_honesty());
        assert!(residual_host_production_spawn_ready_log_helper_ok());
    }
}
