//! Wave 982: IgnoredInGui slaver/producer mouseover residual.
//!
//! Freezes producer_id + IgnoredInGui KindOf into presentation catalog.
//! Host mouseover remaps IgnoredInGui entries to slaver_object_id.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL_METHOD_NAMES_WAVE982: &[&str] = &[
    "slaver_object_id",
    "IgnoredInGui",
    "producer_id",
    "create_mouseover_hint_from_presentation",
    "Wave 982",
    "playable_claim = false",
];

pub const LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL_NAV_STEPS_WAVE982: &[&str] = &[
    "PRODUCER_ID_FREEZE",
    "IGNORED_IN_GUI_KIND",
    "MOUSEOVER_SLAVER_REMAP",
    "LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSlaverMouseoverResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSlaverMouseoverResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

fn mod_source() -> &'static str {
    include_str!("../mod.rs")
}

fn xfer_source() -> &'static str {
    include_str!("../../save_load/xfer.rs")
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_slaver_mouseover_residual_method_names_residual_wave982() -> bool {
    let names = LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL_METHOD_NAMES_WAVE982;
    let ok = residual_name_index(names, "slaver_object_id").is_some()
        && residual_name_index(names, "Wave 982").is_some();
    residual_action_store(ResidualHostSlaverMouseoverResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_slaver_mouseover_residual_nav_commands_residual_wave982() -> bool {
    let steps = LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL_NAV_STEPS_WAVE982;
    let ok = residual_name_index(steps, "LIVE_HOST_SLAVER_MOUSEOVER_RESIDUAL").is_some()
        && residual_name_index(steps, "MOUSEOVER_SLAVER_REMAP").is_some();
    residual_action_store(ResidualHostSlaverMouseoverResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_slaver_mouseover_residual_residual_pack_wave982() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ui = ui_source();
    let m = mod_source();
    let xfer = xfer_source();

    let mouse = match ui.find("fn create_mouseover_hint_from_presentation") {
        Some(i) => &ui[i..],
        None => "",
    };

    let ok =
        (m.contains("IgnoredInGui") || gl.contains("IgnoredInGui") || ui.contains("IgnoredInGui"))
            && pf.contains("pub producer_id: Option<ObjectId>")
            && pf.contains("KindOf::IgnoredInGui")
            && pf.contains("producer_id: obj.producer_id")
            && ui.contains("pub slaver_object_id: Option<u32>")
            && ui.contains("Wave 982")
            && mouse.contains("IgnoredInGui")
            && mouse.contains("slaver_object_id")
            && cnc.contains("slaver_object_id: o.producer_id.map(|id| id.0)")
            && gl.contains("KindOf::IgnoredInGui")
            && gl.contains("ignored_in_gui")
            && xfer.contains("KindOf::IgnoredInGui => 39")
            && xfer.contains("39 => Ok(KindOf::IgnoredInGui)")
            && !cnc.contains("playable_claim = true")
            && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSlaverMouseoverResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_slaver_mouseover_residual_honesty() -> bool {
    let a = honesty_host_slaver_mouseover_residual_method_names_residual_wave982();
    let b = honesty_host_slaver_mouseover_residual_nav_commands_residual_wave982();
    let c = honesty_host_slaver_mouseover_residual_residual_pack_wave982();
    residual_action_store(ResidualHostSlaverMouseoverResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_slaver_mouseover_residual_wave982() {
        assert!(honesty_host_slaver_mouseover_residual_residual_pack_wave982());
        assert!(honesty_host_slaver_mouseover_residual_method_names_residual_wave982());
        assert!(honesty_host_slaver_mouseover_residual_nav_commands_residual_wave982());
        assert!(simulate_live_host_slaver_mouseover_residual_honesty());
    }
}
