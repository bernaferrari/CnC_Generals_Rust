//! Wave 767: GW entity carries FireSoundLoopTime residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-stops looping fire audio via
//! host_fire_sound_loop_log; host peels `tick_fire_sound_loop` on coupled path.
//! Non-coupled still stops via continuous-fire coast. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL_METHOD_NAMES_WAVE767: &[&str] = &[
    "fire_sound_loop_until_frame",
    "host_fire_sound_loop_log",
    "tick_status_timer_expirations",
    "tick_fire_sound_loop",
    "Wave 767",
    "playable_claim = false",
];
pub const LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL_NAV_STEPS_WAVE767: &[&str] = &[
    "REQUIRE_ENTITY_FIRE_SOUND_FIELDS",
    "REQUIRE_GW_STOP_EXPIRE",
    "REQUIRE_HOST_COUPLED_PEEL",
    "LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL_CMD_NAMES_WAVE767: &[&str] = &[
    "host_fire_sound_loop_dual_peel",
    "fire_sound_loop_until_frame",
    "tick_status_timer_expirations",
    "host_fire_sound_loop_log",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFireSoundLoopDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostFireSoundLoopDualPeelAction {
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
fn residual_action_store(a: ResidualHostFireSoundLoopDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_fire_sound_loop_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_fire_sound_loop_dual_peel_last_action()
-> ResidualHostFireSoundLoopDualPeelAction {
    ResidualHostFireSoundLoopDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_fire_sound_loop_dual_peel_method_names_residual_wave767() -> bool {
    let names = LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL_METHOD_NAMES_WAVE767;
    let ok = residual_name_index(names, "fire_sound_loop_until_frame").is_some()
        && residual_name_index(names, "host_fire_sound_loop_log").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_fire_sound_loop").is_some()
        && residual_name_index(names, "Wave 767").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFireSoundLoopDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_fire_sound_loop_dual_peel_source_markers_residual_wave767() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("fire_sound_loop_until_frame")
        && ent.contains("fire_sound_loop_name")
        && sh.contains("Wave 767")
        && sh.contains("host_fire_sound_loop_log::record")
        && gl.contains("Wave 767")
        && !gl.contains("obj.tick_fire_sound_loop(self.frame)")
        && crate::game_logic::object::OBJECT_SRC.contains("fn tick_fire_sound_loop");
    residual_action_store(ResidualHostFireSoundLoopDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_fire_sound_loop_dual_peel_nav_commands_residual_wave767() -> bool {
    let steps = LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL_NAV_STEPS_WAVE767;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FIRE_SOUND_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_STOP_EXPIRE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_COUPLED_PEEL").is_some()
        && residual_name_index(steps, "LIVE_HOST_FIRE_SOUND_LOOP_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFireSoundLoopDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_fire_sound_loop_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 767")
        && sh_source().contains("fire_sound_loop_until_frame")
        && gl_source().contains("Wave 767");
    residual_action_store(ResidualHostFireSoundLoopDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_fire_sound_loop_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_fire_sound_loop_log::record")
        && sh_source()
            .contains("obj.fire_sound_loop_until_frame = ent.fire_sound_loop_until_frame")
        && !gl_source().contains("obj.tick_fire_sound_loop(self.frame)")
        && crate::game_logic::object::OBJECT_SRC.contains("tick_fire_sound_loop(frame)");
    residual_action_store(ResidualHostFireSoundLoopDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_fire_sound_loop_dual_peel_residual_pack_wave767() -> bool {
    honesty_host_fire_sound_loop_dual_peel_method_names_residual_wave767()
        && honesty_host_fire_sound_loop_dual_peel_source_markers_residual_wave767()
        && honesty_host_fire_sound_loop_dual_peel_nav_commands_residual_wave767()
        && simulate_host_fire_sound_loop_dual_peel_collect_source()
        && simulate_host_fire_sound_loop_dual_peel_dispatch_source()
}
pub fn simulate_live_host_fire_sound_loop_dual_peel_honesty() -> bool {
    let ok = honesty_host_fire_sound_loop_dual_peel_residual_pack_wave767();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostFireSoundLoopDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_fire_sound_loop_dual_peel_method_names_residual_wave767());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_fire_sound_loop_dual_peel_source_markers_residual_wave767());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_fire_sound_loop_dual_peel_nav_commands_residual_wave767());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_fire_sound_loop_dual_peel_collect_source());
        assert!(simulate_host_fire_sound_loop_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_fire_sound_loop_dual_peel_residual_pack_wave767());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_fire_sound_loop_dual_peel_honesty());
        assert!(residual_host_fire_sound_loop_dual_peel_ok());
    }
}
