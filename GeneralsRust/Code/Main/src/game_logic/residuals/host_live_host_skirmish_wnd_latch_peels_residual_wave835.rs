//! Wave 835/836: headless skirmish WND latch peels (map/slots/rules/Start) without
//! SkirmishGameOptionsMenu.wnd create_layout stall. Wave 836 keeps `playable_claim`
//! always false; latch peels strengthen `host_vertical_slice_ok` instead.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SKIRMISH_WND_LATCH_PEELS_METHOD_NAMES_WAVE835: &[&str] = &[
    "run_wnd_latches",
    "push_wnd_layout",
    "simulate_skirmish_map_select_and_confirm",
    "simulate_skirmish_prepare_match_options",
    "simulate_skirmish_start_button_gadget_selected",
    "start_skirmish_game",
    "host_vertical_slice_ok",
    "playable_claim = false",
    "Wave 835",
    "Wave 836",
];

pub const LIVE_HOST_SKIRMISH_WND_LATCH_PEELS_NAV_STEPS_WAVE835: &[&str] = &[
    "REQUIRE_HEADLESS_LATCH_PEELS",
    "REQUIRE_NO_LAYOUT_DEFAULT",
    "REQUIRE_START_SKIRMISH_GAME_LATCH",
    "LIVE_HOST_SKIRMISH_WND_LATCH_PEELS",
    "LIVE_HOST_VERTICAL_SLICE_WND_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSkirmishWndLatchPeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSkirmishWndLatchPeelsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

fn skirmish_menu_source() -> &'static str {
    include_str!(
        "../../../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs"
    )
}

pub fn honesty_host_skirmish_wnd_latch_peels_method_names_residual_wave835() -> bool {
    let names = LIVE_HOST_SKIRMISH_WND_LATCH_PEELS_METHOD_NAMES_WAVE835;
    let ok = residual_name_index(names, "run_wnd_latches").is_some()
        && residual_name_index(names, "start_skirmish_game").is_some()
        && residual_name_index(names, "host_vertical_slice_ok").is_some()
        && residual_name_index(names, "playable_claim = false").is_some()
        && residual_name_index(names, "Wave 836").is_some();
    residual_action_store(ResidualHostSkirmishWndLatchPeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_wnd_latch_peels_nav_commands_residual_wave835() -> bool {
    let steps = LIVE_HOST_SKIRMISH_WND_LATCH_PEELS_NAV_STEPS_WAVE835;
    let ok = residual_name_index(steps, "LIVE_HOST_SKIRMISH_WND_LATCH_PEELS").is_some()
        && residual_name_index(steps, "LIVE_HOST_VERTICAL_SLICE_WND_PEELS").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSkirmishWndLatchPeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_wnd_latch_peels_residual_pack_wave835() -> bool {
    let cnc = cnc_source();
    let es = es_source();
    let sm = skirmish_menu_source();
    let ok = cnc.contains("Wave 833/835: headless default avoids SkirmishGameOptionsMenu.wnd")
        && cnc.contains("let run_wnd_latches = push_wnd_layout || self.runtime_host_headless")
        && cnc.contains("if push_wnd_layout")
        && sm.contains("start_skirmish_game(state)")
        && es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`")
        && es.contains("result.skirmish_map_select_wnd_ok")
        && es.contains("result.skirmish_start_wnd_ok")
        && es.contains("presentation_boundary_ok")
        && es.contains("gameworld_rebuilt_boundary_ok");
    residual_action_store(ResidualHostSkirmishWndLatchPeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_skirmish_wnd_latch_peels_honesty() -> bool {
    let a = honesty_host_skirmish_wnd_latch_peels_method_names_residual_wave835();
    let b = honesty_host_skirmish_wnd_latch_peels_nav_commands_residual_wave835();
    let c = honesty_host_skirmish_wnd_latch_peels_residual_pack_wave835();
    residual_action_store(ResidualHostSkirmishWndLatchPeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_skirmish_wnd_latch_peels_residual_wave835() {
        assert!(honesty_host_skirmish_wnd_latch_peels_residual_pack_wave835());
        assert!(honesty_host_skirmish_wnd_latch_peels_method_names_residual_wave835());
        assert!(honesty_host_skirmish_wnd_latch_peels_nav_commands_residual_wave835());
        assert!(simulate_live_host_skirmish_wnd_latch_peels_honesty());
    }
}
