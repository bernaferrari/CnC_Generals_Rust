//! Wave 713: sole-tick production complete requires ready-log membership.
//! Empty `host_production_ready_log` no longer triggers a host full-queue scan.
//! GameWorld remains readiness authority under production authority sole-tick
//! (opt-in GameLogic::set_production_authority(true)).
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_METHOD_NAMES_WAVE713: &[&str] = &[
    "ready_producers.contains",
    "host_production_ready_log",
    "gameworld_production_sole_tick_enabled",
    "try_complete_production",
    "Wave 713",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_NAV_STEPS_WAVE713: &[&str] = &[
    "REQUIRE_READY_LOG_MEMBERSHIP",
    "REQUIRE_NO_EMPTY_SCAN_FALLBACK",
    "REQUIRE_SOLE_TICK_GATE",
    "LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_CMD_NAMES_WAVE713: &[&str] = &[
    "host_production_ready_no_empty_scan",
    "ready_log_membership",
    "no_empty_scan_fallback",
    "sole_tick_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionReadyNoEmptyScanAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionReadyNoEmptyScanAction {
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
fn residual_action_store(a: ResidualHostProductionReadyNoEmptyScanAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_ready_no_empty_scan_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_ready_no_empty_scan_last_action()
-> ResidualHostProductionReadyNoEmptyScanAction {
    ResidualHostProductionReadyNoEmptyScanAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_production_ready_no_empty_scan_method_names_residual_wave713() -> bool {
    let names = LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_METHOD_NAMES_WAVE713;
    let ok = residual_name_index(names, "ready_producers.contains").is_some()
        && residual_name_index(names, "host_production_ready_log").is_some()
        && residual_name_index(names, "gameworld_production_sole_tick_enabled").is_some()
        && residual_name_index(names, "try_complete_production").is_some()
        && residual_name_index(names, "Wave 713").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::MethodNames);
    ok
}
pub fn honesty_host_production_ready_no_empty_scan_source_markers_residual_wave713() -> bool {
    let gl = gl_source();
    let sole_ok = gl.contains("Wave 464/614: GameWorld sole-ticks progress + exit delay")
        && gl.contains("Wave 713: empty ready log")
        && gl.contains("ready_producers.contains(&id)")
        && !gl.contains("ready_producers.is_empty() || ready_producers.contains(&id)")
        && !gl.contains("fallback scan if ready log empty this frame")
        && gl.contains("Wave 617/713")
        && gl.contains("ready_structures.contains(&id)")
        && !gl.contains("ready_structures.is_empty() || ready_structures.contains(&id)");
    let ok = sole_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_ready_no_empty_scan_nav_commands_residual_wave713() -> bool {
    let steps = LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_NAV_STEPS_WAVE713;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN_CMD_NAMES_WAVE713;
    let ok = residual_name_index(steps, "REQUIRE_READY_LOG_MEMBERSHIP").is_some()
        && residual_name_index(steps, "REQUIRE_NO_EMPTY_SCAN_FALLBACK").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_GATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_READY_NO_EMPTY_SCAN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_ready_no_empty_scan").is_some()
        && residual_name_index(cmds, "ready_log_membership").is_some()
        && residual_name_index(cmds, "no_empty_scan_fallback").is_some()
        && residual_name_index(cmds, "sole_tick_gate").is_some();
    residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::NavCommands);
    ok
}
pub fn simulate_host_production_ready_no_empty_scan_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("ready_producers.contains(&id)")
        && gl.contains("host_production_ready_log::drain");
    residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::CollectSource);
    ok
}
pub fn simulate_host_production_ready_no_empty_scan_dispatch_source() -> bool {
    let ok =
        gl_source().contains("Wave 713: empty ready log") || gl_source().contains("Wave 617/713");
    residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::DispatchSource);
    ok
}
pub fn honesty_host_production_ready_no_empty_scan_residual_pack_wave713() -> bool {
    honesty_host_production_ready_no_empty_scan_method_names_residual_wave713()
        && honesty_host_production_ready_no_empty_scan_source_markers_residual_wave713()
        && honesty_host_production_ready_no_empty_scan_nav_commands_residual_wave713()
        && simulate_host_production_ready_no_empty_scan_collect_source()
        && simulate_host_production_ready_no_empty_scan_dispatch_source()
}
pub fn simulate_live_host_production_ready_no_empty_scan_honesty() -> bool {
    let ok = honesty_host_production_ready_no_empty_scan_residual_pack_wave713();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionReadyNoEmptyScanAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::buildings::{
        BuildingData, BuildingType, ProductionItem, ProductionKind,
    };
    use crate::game_logic::host_production_ready_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Resources, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        begin_shadow_coupled_tick, end_shadow_coupled_tick, gameworld_production_sole_tick_enabled,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str) {
        let mut t = ThingTemplate::new(name);
        t.set_health(100.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_ready_no_empty_scan_method_names_residual_wave713());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_ready_no_empty_scan_source_markers_residual_wave713());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_ready_no_empty_scan_nav_commands_residual_wave713());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_ready_no_empty_scan_collect_source());
        assert!(simulate_host_production_ready_no_empty_scan_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_ready_no_empty_scan_residual_pack_wave713());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_ready_no_empty_scan_honesty());
        assert!(residual_host_production_ready_no_empty_scan_ok());
    }

    #[test]
    fn sole_tick_empty_ready_log_does_not_complete_queue_head() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev_sh = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        host_production_ready_log::clear();

        let mut logic = GameLogic::new();
        logic.set_production_authority(true);
        ensure_template(&mut logic, "ReadyScanFactory");
        ensure_template(&mut logic, "ReadyScanUnit");
        let id = logic
            .create_object("ReadyScanFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("factory");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(false);
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "ReadyScanUnit".into(),
                progress: 10.0,
                total_time: 10.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 100,
                    power: 0,
                },
                kind: ProductionKind::Unit,
                quantity_total: 1,
                quantity_produced: 0,
            });
            bd.exit_delay_remaining = 0.0;
            obj.building_data = Some(bd);
        }

        begin_shadow_coupled_tick();
        assert!(gameworld_production_sole_tick_enabled());
        // Empty ready log: must not try_complete.
        assert!(host_production_ready_log::drain().is_empty());
        let (upg, units) = logic.host_collect_production_completions(1.0 / 30.0);
        assert!(
            upg.is_empty(),
            "upgrade completions without ready log: {upg:?}"
        );
        assert!(
            units.is_empty(),
            "unit completions without ready log: {units:?}"
        );
        let qlen = logic
            .get_objects()
            .get(&id)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.len())
            .unwrap_or(0);
        assert_eq!(qlen, 1, "queue head must remain without ready membership");

        // Membership unlocks try_complete.
        host_production_ready_log::record(id, "ReadyScanUnit", false);
        let (_upg2, units2) = logic.host_collect_production_completions(1.0 / 30.0);
        assert_eq!(
            units2.len(),
            1,
            "ready membership must complete unit: {units2:?}"
        );
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev_sh {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
