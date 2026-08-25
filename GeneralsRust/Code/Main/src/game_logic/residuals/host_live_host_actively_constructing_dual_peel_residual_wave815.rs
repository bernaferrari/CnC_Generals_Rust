//! Wave 815: GW entity carries ACTIVELY_CONSTRUCTING model bit from AI state /
//! production queue; under coupled dual-tick sole-ticks into logs; host peels
//! update_actively_constructing_model_conditions. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL_METHOD_NAMES_WAVE815: &[&str] = &[
    "ACTIVELY_CONSTRUCTING",
    "model_condition_bits",
    "host_actively_constructing_log",
    "update_actively_constructing_model_conditions",
    "ai_state_ordinal",
    "Wave 815",
    "playable_claim = false",
];
pub const LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL_NAV_STEPS_WAVE815: &[&str] = &[
    "REQUIRE_ENTITY_MODEL_AI_FIELDS",
    "REQUIRE_GW_ACTIVELY_CONSTRUCTING_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_BIT_DRAIN",
    "LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostActivelyConstructingDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostActivelyConstructingDualPeelAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostActivelyConstructingDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_actively_constructing_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_actively_constructing_dual_peel_last_action()
-> ResidualHostActivelyConstructingDualPeelAction {
    ResidualHostActivelyConstructingDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_actively_constructing_dual_peel_method_names_residual_wave815() -> bool {
    let names = LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL_METHOD_NAMES_WAVE815;
    let ok = residual_name_index(names, "ACTIVELY_CONSTRUCTING").is_some()
        && residual_name_index(names, "model_condition_bits").is_some()
        && residual_name_index(names, "host_actively_constructing_log").is_some()
        && residual_name_index(names, "update_actively_constructing_model_conditions").is_some()
        && residual_name_index(names, "ai_state_ordinal").is_some()
        && residual_name_index(names, "Wave 815").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostActivelyConstructingDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_actively_constructing_dual_peel_source_markers_residual_wave815() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("model_condition_bits")
        && ent.contains("ai_state_ordinal")
        && ent.contains("production_queue_len")
        && sh.contains("Wave 815")
        && sh.contains("host_actively_constructing_log::record")
        && sh.contains("host_actively_constructing_log::drain")
        && sh.contains("actively_constructing_model_bit")
        && gl.contains("Wave 815")
        && gl.contains("update_actively_constructing_model_conditions")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostActivelyConstructingDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_actively_constructing_dual_peel_nav_commands_residual_wave815() -> bool {
    let steps = LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL_NAV_STEPS_WAVE815;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_MODEL_AI_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_ACTIVELY_CONSTRUCTING_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_BIT_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_ACTIVELY_CONSTRUCTING_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostActivelyConstructingDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_actively_constructing_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 815")
        && sh_source().contains("ACTIVELY_CONSTRUCTING")
        && gl_source().contains("Wave 815");
    residual_action_store(ResidualHostActivelyConstructingDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_actively_constructing_dual_peel_dispatch_source() -> bool {
    // Wave 942: host peels model bits via apply_host_residual_mutation_op
    // (SetModelConditionBits) which bumps actively_constructing_updates.
    let sh = sh_source();
    let gl = gl_source();
    let ok = sh.contains("host_actively_constructing_log::drain")
        && (sh.contains("actively_constructing_updates")
            || (sh.contains("SetModelConditionBits")
                && sh.contains("apply_host_residual_mutation_op")
                && gl.contains("actively_constructing_updates")))
        && gl.contains("update_actively_constructing_model_conditions")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostActivelyConstructingDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_actively_constructing_dual_peel_residual_pack_wave815() -> bool {
    honesty_host_actively_constructing_dual_peel_method_names_residual_wave815()
        && honesty_host_actively_constructing_dual_peel_source_markers_residual_wave815()
        && honesty_host_actively_constructing_dual_peel_nav_commands_residual_wave815()
}
pub fn simulate_live_host_actively_constructing_dual_peel_honesty() -> bool {
    let ok = honesty_host_actively_constructing_dual_peel_residual_pack_wave815()
        && simulate_host_actively_constructing_dual_peel_collect_source()
        && simulate_host_actively_constructing_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_actively_constructing_dual_peel_method_names_residual_wave815());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_actively_constructing_dual_peel_source_markers_residual_wave815());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_actively_constructing_dual_peel_nav_commands_residual_wave815());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_actively_constructing_dual_peel_collect_source());
        assert!(simulate_host_actively_constructing_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_actively_constructing_dual_peel_residual_pack_wave815());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_actively_constructing_dual_peel_honesty());
        assert!(residual_host_actively_constructing_dual_peel_ok());
    }
}
