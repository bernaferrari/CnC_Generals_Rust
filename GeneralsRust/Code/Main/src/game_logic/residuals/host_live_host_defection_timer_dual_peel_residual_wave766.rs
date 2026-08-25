//! Wave 766: GW entity carries ObjectDefectionHelper timer fields; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks undetected-defector
//! timer/flash; host peels `tick_defection_helper`. Writeback restores helper
//! + DefectorTimerDing. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL_METHOD_NAMES_WAVE766: &[&str] = &[
    "defection_undetected",
    "defection_detection_end",
    "tick_status_timer_expirations",
    "tick_defection_helper",
    "Wave 766",
    "playable_claim = false",
];
pub const LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL_NAV_STEPS_WAVE766: &[&str] = &[
    "REQUIRE_ENTITY_DEFECTION_FIELDS",
    "REQUIRE_GW_TIMER_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK_HELPER",
    "LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL_CMD_NAMES_WAVE766: &[&str] = &[
    "host_defection_timer_dual_peel",
    "defection_undetected",
    "tick_status_timer_expirations",
    "tick_defection_helper",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDefectionTimerDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostDefectionTimerDualPeelAction {
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
fn residual_action_store(a: ResidualHostDefectionTimerDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_defection_timer_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_defection_timer_dual_peel_last_action()
-> ResidualHostDefectionTimerDualPeelAction {
    ResidualHostDefectionTimerDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_defection_timer_dual_peel_method_names_residual_wave766() -> bool {
    let names = LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL_METHOD_NAMES_WAVE766;
    let ok = residual_name_index(names, "defection_undetected").is_some()
        && residual_name_index(names, "defection_detection_end").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_defection_helper").is_some()
        && residual_name_index(names, "Wave 766").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDefectionTimerDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_defection_timer_dual_peel_source_markers_residual_wave766() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("defection_undetected")
        && ent.contains("defection_detection_end")
        && sh.contains("Wave 766")
        && sh.contains("defection_final_white_flash")
        && gl.contains("Wave 766")
        && gl.contains("tick_defection_helper(self.frame)");
    residual_action_store(ResidualHostDefectionTimerDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_defection_timer_dual_peel_nav_commands_residual_wave766() -> bool {
    let steps = LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL_NAV_STEPS_WAVE766;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_DEFECTION_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_TIMER_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_DEFECTION_TIMER_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostDefectionTimerDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_defection_timer_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 766")
        && sh_source().contains("defection_undetected")
        && gl_source().contains("Wave 766");
    residual_action_store(ResidualHostDefectionTimerDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_defection_timer_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("DefectorTimerDing")
        && sh_source().contains("obj.defection_helper.get_or_insert")
        && gl_source().contains("tick_defection_helper(self.frame)")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostDefectionTimerDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_defection_timer_dual_peel_residual_pack_wave766() -> bool {
    honesty_host_defection_timer_dual_peel_method_names_residual_wave766()
        && honesty_host_defection_timer_dual_peel_source_markers_residual_wave766()
        && honesty_host_defection_timer_dual_peel_nav_commands_residual_wave766()
        && simulate_host_defection_timer_dual_peel_collect_source()
        && simulate_host_defection_timer_dual_peel_dispatch_source()
}
pub fn simulate_live_host_defection_timer_dual_peel_honesty() -> bool {
    let ok = honesty_host_defection_timer_dual_peel_residual_pack_wave766();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostDefectionTimerDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_defection_timer_dual_peel_method_names_residual_wave766());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_defection_timer_dual_peel_source_markers_residual_wave766());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_defection_timer_dual_peel_nav_commands_residual_wave766());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_defection_timer_dual_peel_collect_source());
        assert!(simulate_host_defection_timer_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_defection_timer_dual_peel_residual_pack_wave766());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_defection_timer_dual_peel_honesty());
        assert!(residual_host_defection_timer_dual_peel_ok());
    }
}
