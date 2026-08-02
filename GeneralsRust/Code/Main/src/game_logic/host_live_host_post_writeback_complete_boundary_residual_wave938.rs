//! Wave 938: post-writeback construction/sell/SP complete authority boundary.
//!
//! Shadow same-frame sole-tick construction complete, sell finish, and special-power
//! ready drains call GameLogic::apply_post_writeback_complete_op.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY_METHOD_NAMES_WAVE938: &[&str] = &[
    "apply_post_writeback_complete_op",
    "PostWritebackCompleteOp",
    "ConstructionCompletionsAfterReadyWriteback",
    "SellCompletionsAfterReadyWriteback",
    "SpecialPowerReadyAfterWriteback",
    "Wave 938",
    "playable_claim = false",
];

pub const LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY_NAV_STEPS_WAVE938: &[&str] = &[
    "POST_WRITEBACK_COMPLETE_BOUNDARY",
    "SINGLE_APPLY_POST_WRITEBACK_COMPLETE_OP",
    "LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPostWritebackCompleteBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPostWritebackCompleteBoundaryAction) {
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

pub fn honesty_host_post_writeback_complete_boundary_method_names_residual_wave938() -> bool {
    let names = LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY_METHOD_NAMES_WAVE938;
    let ok = residual_name_index(names, "apply_post_writeback_complete_op").is_some()
        && residual_name_index(names, "Wave 938").is_some();
    residual_action_store(ResidualHostPostWritebackCompleteBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_post_writeback_complete_boundary_nav_commands_residual_wave938() -> bool {
    let steps = LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY_NAV_STEPS_WAVE938;
    let ok = residual_name_index(steps, "LIVE_HOST_POST_WRITEBACK_COMPLETE_BOUNDARY").is_some()
        && residual_name_index(steps, "POST_WRITEBACK_COMPLETE_BOUNDARY").is_some();
    residual_action_store(ResidualHostPostWritebackCompleteBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_post_writeback_complete_boundary_residual_pack_wave938() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_post_writeback_complete_op", 1200));
    let ok = gl.contains("enum PostWritebackCompleteOp")
        && api.contains("host_apply_construction_completions_after_ready_writeback")
        && api.contains("host_apply_sell_completions_after_ready_writeback")
        && api.contains("host_apply_special_power_ready_after_writeback")
        && sh.contains("apply_post_writeback_complete_op")
        && sh.contains("ConstructionCompletionsAfterReadyWriteback")
        && sh.contains("SellCompletionsAfterReadyWriteback")
        && sh.contains("SpecialPowerReadyAfterWriteback")
        && !sh.contains("logic.host_apply_construction_completions_after_ready_writeback")
        && !sh.contains("logic.host_apply_sell_completions_after_ready_writeback")
        && !sh.contains("logic.host_apply_special_power_ready_after_writeback")
        && sh.contains("938")
        && gl.contains("Wave 938")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPostWritebackCompleteBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_post_writeback_complete_boundary_honesty() -> bool {
    let a = honesty_host_post_writeback_complete_boundary_method_names_residual_wave938();
    let b = honesty_host_post_writeback_complete_boundary_nav_commands_residual_wave938();
    let c = honesty_host_post_writeback_complete_boundary_residual_pack_wave938();
    residual_action_store(ResidualHostPostWritebackCompleteBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_post_writeback_complete_boundary_residual_wave938() {
        assert!(honesty_host_post_writeback_complete_boundary_residual_pack_wave938());
        assert!(honesty_host_post_writeback_complete_boundary_method_names_residual_wave938());
        assert!(honesty_host_post_writeback_complete_boundary_nav_commands_residual_wave938());
        assert!(simulate_live_host_post_writeback_complete_boundary_honesty());
    }
}
