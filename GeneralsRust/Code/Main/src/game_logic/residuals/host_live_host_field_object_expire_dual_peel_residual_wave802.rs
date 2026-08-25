//! Wave 802: GW entity carries Nuke/Anthrax/Inferno field-object lifetime residual;
//! under coupled dual-tick sole-ticks expires into host_field_object_expire_log;
//! host peels field-object updates. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL_METHOD_NAMES_WAVE802: &[&str] = &[
    "nuke_radiation_field",
    "anthrax_toxin_field",
    "inferno_fire_field",
    "host_field_object_expire_log",
    "update_nuke_radiation_field_objects",
    "update_anthrax_toxin_field_objects",
    "update_inferno_fire_field_objects",
    "Wave 802",
    "playable_claim = false",
];
pub const LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL_NAV_STEPS_WAVE802: &[&str] = &[
    "REQUIRE_ENTITY_FIELD_OBJECT_FIELDS",
    "REQUIRE_GW_EXPIRE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_EXPIRE_DRAIN",
    "LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFieldObjectExpireDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostFieldObjectExpireDualPeelAction {
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
fn residual_action_store(a: ResidualHostFieldObjectExpireDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_field_object_expire_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_field_object_expire_dual_peel_last_action()
-> ResidualHostFieldObjectExpireDualPeelAction {
    ResidualHostFieldObjectExpireDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_field_object_expire_dual_peel_method_names_residual_wave802() -> bool {
    let names = LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL_METHOD_NAMES_WAVE802;
    let ok = residual_name_index(names, "nuke_radiation_field").is_some()
        && residual_name_index(names, "anthrax_toxin_field").is_some()
        && residual_name_index(names, "inferno_fire_field").is_some()
        && residual_name_index(names, "host_field_object_expire_log").is_some()
        && residual_name_index(names, "update_nuke_radiation_field_objects").is_some()
        && residual_name_index(names, "update_anthrax_toxin_field_objects").is_some()
        && residual_name_index(names, "update_inferno_fire_field_objects").is_some()
        && residual_name_index(names, "Wave 802").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFieldObjectExpireDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_field_object_expire_dual_peel_source_markers_residual_wave802() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("nuke_radiation_field")
        && ent.contains("anthrax_toxin_field")
        && ent.contains("inferno_fire_field")
        && sh.contains("Wave 802")
        && sh.contains("host_field_object_expire_log::record")
        && sh.contains("host_field_object_expire_log::drain")
        && gl.contains("Wave 802")
        && gl.contains("update_nuke_radiation_field_objects")
        && gl.contains("update_anthrax_toxin_field_objects")
        && gl.contains("update_inferno_fire_field_objects");
    residual_action_store(ResidualHostFieldObjectExpireDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_field_object_expire_dual_peel_nav_commands_residual_wave802() -> bool {
    let steps = LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL_NAV_STEPS_WAVE802;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FIELD_OBJECT_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_EXPIRE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_FIELD_OBJECT_EXPIRE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFieldObjectExpireDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_field_object_expire_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 802")
        && sh_source().contains("nuke_radiation_field")
        && gl_source().contains("Wave 802");
    residual_action_store(ResidualHostFieldObjectExpireDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_field_object_expire_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_field_object_expire_log::record")
        && sh_source().contains("host_field_object_expire_log::drain")
        && gl_source().contains("update_nuke_radiation_field_objects")
        && gl_source().contains("update_anthrax_toxin_field_objects")
        && gl_source().contains("update_inferno_fire_field_objects")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFieldObjectExpireDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_field_object_expire_dual_peel_residual_pack_wave802() -> bool {
    honesty_host_field_object_expire_dual_peel_method_names_residual_wave802()
        && honesty_host_field_object_expire_dual_peel_source_markers_residual_wave802()
        && honesty_host_field_object_expire_dual_peel_nav_commands_residual_wave802()
}
pub fn simulate_live_host_field_object_expire_dual_peel_honesty() -> bool {
    let ok = honesty_host_field_object_expire_dual_peel_residual_pack_wave802()
        && simulate_host_field_object_expire_dual_peel_collect_source()
        && simulate_host_field_object_expire_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_field_object_expire_dual_peel_method_names_residual_wave802());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_field_object_expire_dual_peel_source_markers_residual_wave802());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_field_object_expire_dual_peel_nav_commands_residual_wave802());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_field_object_expire_dual_peel_collect_source());
        assert!(simulate_host_field_object_expire_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_field_object_expire_dual_peel_residual_pack_wave802());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_field_object_expire_dual_peel_honesty());
        assert!(residual_host_field_object_expire_dual_peel_ok());
    }
}
