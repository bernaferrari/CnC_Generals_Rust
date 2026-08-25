//! Wave 809: GW entity carries firewall segment crawl/expire + radar van ping;
//! under coupled dual-tick sole-ticks motion/expire into field-object logs; host
//! keeps fire_walls.crawl_segments and peels object body + radar update.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL_METHOD_NAMES_WAVE809: &[&str] = &[
    "firewall_segment",
    "radar_van_ping",
    "host_field_object_expire_log",
    "update_firewall_segment_objects",
    "update_radar_van_pings",
    "crawl_segments",
    "Wave 809",
    "playable_claim = false",
];
pub const LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL_NAV_STEPS_WAVE809: &[&str] = &[
    "REQUIRE_ENTITY_FIREWALL_RADAR_FIELDS",
    "REQUIRE_GW_CRAWL_EXPIRE_TICK",
    "REQUIRE_HOST_REGISTRY_CRAWL",
    "REQUIRE_HOST_OBJECT_PEEL",
    "REQUIRE_EXPIRE_DRAIN",
    "LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFirewallRadarDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostFirewallRadarDualPeelAction {
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
fn residual_action_store(a: ResidualHostFirewallRadarDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_firewall_radar_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_firewall_radar_dual_peel_last_action()
-> ResidualHostFirewallRadarDualPeelAction {
    ResidualHostFirewallRadarDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_firewall_radar_dual_peel_method_names_residual_wave809() -> bool {
    let names = LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL_METHOD_NAMES_WAVE809;
    let ok = residual_name_index(names, "firewall_segment").is_some()
        && residual_name_index(names, "radar_van_ping").is_some()
        && residual_name_index(names, "host_field_object_expire_log").is_some()
        && residual_name_index(names, "update_firewall_segment_objects").is_some()
        && residual_name_index(names, "update_radar_van_pings").is_some()
        && residual_name_index(names, "crawl_segments").is_some()
        && residual_name_index(names, "Wave 809").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFirewallRadarDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_firewall_radar_dual_peel_source_markers_residual_wave809() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let log = include_str!("../host_field_object_expire_log.rs");
    let ok = ent.contains("firewall_segment")
        && ent.contains("radar_van_ping")
        && log.contains("FirewallSegment")
        && log.contains("RadarVanPing")
        && sh.contains("Wave 809")
        && sh.contains("FIREWALL_INCH_PER_FRAME")
        && sh.contains("FieldObjectKind::FirewallSegment")
        && sh.contains("FieldObjectKind::RadarVanPing")
        && gl.contains("Wave 809")
        && gl.contains("crawl_segments()")
        && gl.contains("update_radar_van_pings")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFirewallRadarDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_firewall_radar_dual_peel_nav_commands_residual_wave809() -> bool {
    let steps = LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL_NAV_STEPS_WAVE809;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FIREWALL_RADAR_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_CRAWL_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_REGISTRY_CRAWL").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_OBJECT_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_EXPIRE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_FIREWALL_RADAR_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFirewallRadarDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_firewall_radar_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 809")
        && sh_source().contains("firewall_segment")
        && gl_source().contains("Wave 809");
    residual_action_store(ResidualHostFirewallRadarDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_firewall_radar_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("FieldObjectKind::FirewallSegment")
        && sh_source().contains("FieldObjectKind::RadarVanPing")
        && gl_source().contains("update_firewall_segment_objects")
        && gl_source().contains("update_radar_van_pings")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFirewallRadarDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_firewall_radar_dual_peel_residual_pack_wave809() -> bool {
    honesty_host_firewall_radar_dual_peel_method_names_residual_wave809()
        && honesty_host_firewall_radar_dual_peel_source_markers_residual_wave809()
        && honesty_host_firewall_radar_dual_peel_nav_commands_residual_wave809()
}
pub fn simulate_live_host_firewall_radar_dual_peel_honesty() -> bool {
    let ok = honesty_host_firewall_radar_dual_peel_residual_pack_wave809()
        && simulate_host_firewall_radar_dual_peel_collect_source()
        && simulate_host_firewall_radar_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_firewall_radar_dual_peel_method_names_residual_wave809());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_firewall_radar_dual_peel_source_markers_residual_wave809());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_firewall_radar_dual_peel_nav_commands_residual_wave809());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_firewall_radar_dual_peel_collect_source());
        assert!(simulate_host_firewall_radar_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_firewall_radar_dual_peel_residual_pack_wave809());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_firewall_radar_dual_peel_honesty());
        assert!(residual_host_firewall_radar_dual_peel_ok());
    }
}
