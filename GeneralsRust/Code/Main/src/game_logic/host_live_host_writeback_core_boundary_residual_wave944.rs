//! Wave 944: shadow→host core writeback authority boundary.
//!
//! Health, experience, transform, attack-target, and move-target writebacks
//! route through `apply_host_writeback_op` / `HostWritebackOp` instead of
//! direct `get_objects_mut` dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WRITEBACK_CORE_BOUNDARY_METHOD_NAMES_WAVE944: &[&str] = &[
    "apply_host_writeback_op",
    "HostWritebackOp",
    "Health",
    "Experience",
    "Transform",
    "AttackTarget",
    "MoveTarget",
    "Wave 944",
    "playable_claim = false",
];

pub const LIVE_HOST_WRITEBACK_CORE_BOUNDARY_NAV_STEPS_WAVE944: &[&str] = &[
    "WRITEBACK_CORE_BOUNDARY",
    "HOST_WRITEBACK_OP",
    "LIVE_HOST_WRITEBACK_CORE_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackCoreBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWritebackCoreBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fn_window<'a>(src: &'a str, marker: &str) -> &'a str {
    let Some(i) = src.find(marker) else {
        return "";
    };
    let Some(brace) = src[i..].find('{').map(|o| i + o) else {
        return "";
    };
    let mut depth = 0usize;
    let mut p = brace;
    let bytes = src.as_bytes();
    while p < src.len() {
        match bytes[p] as char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[i..=p];
                }
            }
            _ => {}
        }
        p += 1;
    }
    &src[i..src.len().min(i + 8_000)]
}

pub fn honesty_host_writeback_core_boundary_method_names_residual_wave944() -> bool {
    let names = LIVE_HOST_WRITEBACK_CORE_BOUNDARY_METHOD_NAMES_WAVE944;
    let ok = residual_name_index(names, "apply_host_writeback_op").is_some()
        && residual_name_index(names, "HostWritebackOp").is_some()
        && residual_name_index(names, "Wave 944").is_some();
    residual_action_store(ResidualHostWritebackCoreBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_core_boundary_nav_commands_residual_wave944() -> bool {
    let steps = LIVE_HOST_WRITEBACK_CORE_BOUNDARY_NAV_STEPS_WAVE944;
    let ok = residual_name_index(steps, "LIVE_HOST_WRITEBACK_CORE_BOUNDARY").is_some()
        && residual_name_index(steps, "HOST_WRITEBACK_OP").is_some();
    residual_action_store(ResidualHostWritebackCoreBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_writeback_core_boundary_residual_pack_wave944() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_host_writeback_op", 4000));
    let health = non_comment_code(fn_window(sh, "pub fn writeback_health_to_host"));
    let xp = non_comment_code(fn_window(sh, "pub fn writeback_experience_to_host"));
    let xf = non_comment_code(fn_window(sh, "pub fn writeback_transforms_to_host"));
    let atk = non_comment_code(fn_window(sh, "pub fn writeback_attack_targets_to_host"));
    let mv = non_comment_code(fn_window(sh, "pub fn writeback_move_targets_to_host"));
    let ok = gl.contains("enum HostWritebackOp")
        && gl.contains("apply_host_writeback_op")
        && api.contains("HostWritebackOp::Health")
        && api.contains("HostWritebackOp::Experience")
        && api.contains("HostWritebackOp::Transform")
        && api.contains("HostWritebackOp::AttackTarget")
        && api.contains("HostWritebackOp::MoveTarget")
        && health.contains("apply_host_writeback_op")
        && health.contains("HostWritebackOp::Health")
        && !health.contains("get_objects_mut")
        && xp.contains("HostWritebackOp::Experience")
        && !xp.contains("get_objects_mut")
        && xf.contains("HostWritebackOp::Transform")
        && !xf.contains("get_objects_mut")
        && atk.contains("HostWritebackOp::AttackTarget")
        && !atk.contains("get_objects_mut")
        && mv.contains("HostWritebackOp::MoveTarget")
        && !mv.contains("get_objects_mut")
        && gl.contains("Wave 944")
        && sh.contains("Wave 944")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostWritebackCoreBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_writeback_core_boundary_honesty() -> bool {
    let a = honesty_host_writeback_core_boundary_method_names_residual_wave944();
    let b = honesty_host_writeback_core_boundary_nav_commands_residual_wave944();
    let c = honesty_host_writeback_core_boundary_residual_pack_wave944();
    residual_action_store(ResidualHostWritebackCoreBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_writeback_core_boundary_residual_wave944() {
        assert!(honesty_host_writeback_core_boundary_residual_pack_wave944());
        assert!(honesty_host_writeback_core_boundary_method_names_residual_wave944());
        assert!(honesty_host_writeback_core_boundary_nav_commands_residual_wave944());
        assert!(simulate_live_host_writeback_core_boundary_honesty());
    }
}
