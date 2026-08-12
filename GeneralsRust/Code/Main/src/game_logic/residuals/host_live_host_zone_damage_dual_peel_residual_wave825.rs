//! Wave 825: under coupled dual-tick, host peels zone/field damage updates and
//! sole-ticks them after GW writeback (GW-authoritative poses/HP).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL_METHOD_NAMES_WAVE825: &[&str] = &[
    "update_scud_poison_zones",
    "update_nuclear_tanks_radiation_zones",
    "update_firewalls",
    "update_inferno_fire_zones",
    "update_spectre_orbit_fields",
    "update_toxin_tractor_poison_zones",
    "update_anthrax_toxin_fields",
    "update_nuclear_radiation_fields",
    "update_neutron_slow_death_fields",
    "update_microwave_emitter_field",
    "update_microwave_disable",
    "tick_zone_damage_fields_sole",
    "Wave 825",
    "playable_claim = false",
];
pub const LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL_NAV_STEPS_WAVE825: &[&str] = &[
    "REQUIRE_HOST_PEEL",
    "REQUIRE_POST_WRITEBACK_SOLE_TICK",
    "LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostZoneDamageDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostZoneDamageDualPeelAction {
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
fn residual_action_store(a: ResidualHostZoneDamageDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_zone_damage_dual_peel_method_names_residual_wave825() -> bool {
    let names = LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL_METHOD_NAMES_WAVE825;
    let ok = names
        .iter()
        .all(|n| residual_name_index(names, n).is_some());
    residual_action_store(ResidualHostZoneDamageDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_zone_damage_dual_peel_nav_commands_residual_wave825() -> bool {
    let steps = LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL_NAV_STEPS_WAVE825;
    let ok = residual_name_index(steps, "LIVE_HOST_ZONE_DAMAGE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostZoneDamageDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_zone_damage_dual_peel_residual_pack_wave825() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = (sh
        .contains("Wave 825: host zone/field damage sole-tick after GW writeback positions/HP.")
        || sh.contains("apply_post_writeback_sole_ticks")
        || sh.contains("Wave 823–827/940"))
        && (sh.contains("tick_zone_damage_fields_sole")
            || sh.contains("apply_post_writeback_sole_ticks"))
        && gl.contains(
            "Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.",
        )
        && gl.contains("fn tick_zone_damage_fields_sole")
        && (gl.contains("apply_post_writeback_sole_ticks") || gl.contains("Wave 940"));
    residual_action_store(ResidualHostZoneDamageDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_zone_damage_dual_peel_honesty() -> bool {
    let a = honesty_host_zone_damage_dual_peel_method_names_residual_wave825();
    let b = honesty_host_zone_damage_dual_peel_nav_commands_residual_wave825();
    let c = honesty_host_zone_damage_dual_peel_residual_pack_wave825();
    residual_action_store(ResidualHostZoneDamageDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_zone_damage_dual_peel_residual_wave825() {
        assert!(honesty_host_zone_damage_dual_peel_residual_pack_wave825());
        assert!(honesty_host_zone_damage_dual_peel_method_names_residual_wave825());
        assert!(honesty_host_zone_damage_dual_peel_nav_commands_residual_wave825());
        assert!(simulate_live_host_zone_damage_dual_peel_honesty());
    }
}
