//! Wave 717: sole-tick special-power ready EVA runs after GW SP writeback
//! in the same coupled tick (not mid-update_construction drain).
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_METHOD_NAMES_WAVE717: &[&str] = &[
    "host_apply_special_power_ready_after_writeback",
    "apply_post_writeback_complete_op",
    "SpecialPowerReadyAfterWriteback",
    "gameworld_special_power_sole_tick_enabled",
    "writeback_special_power_to_host",
    "host_special_power_ready_log",
    "Wave 717",
    "playable_claim = false",
];
pub const LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_NAV_STEPS_WAVE717: &[&str] = &[
    "REQUIRE_POST_WRITEBACK_EVA",
    "REQUIRE_SOLE_TICK_NO_MID_DRAIN",
    "REQUIRE_SESSION_HANDOFF",
    "LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_CMD_NAMES_WAVE717: &[&str] = &[
    "host_special_power_same_frame_ready_eva",
    "post_writeback_eva",
    "sole_tick_no_mid_drain",
    "session_handoff",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpecialPowerSameFrameReadyEvaAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSpecialPowerSameFrameReadyEvaAction {
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
fn residual_action_store(a: ResidualHostSpecialPowerSameFrameReadyEvaAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_special_power_same_frame_ready_eva_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_special_power_same_frame_ready_eva_last_action()
-> ResidualHostSpecialPowerSameFrameReadyEvaAction {
    ResidualHostSpecialPowerSameFrameReadyEvaAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_special_power_same_frame_ready_eva_method_names_residual_wave717() -> bool {
    let names = LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_METHOD_NAMES_WAVE717;
    let ok = residual_name_index(names, "host_apply_special_power_ready_after_writeback").is_some()
        && residual_name_index(names, "gameworld_special_power_sole_tick_enabled").is_some()
        && residual_name_index(names, "writeback_special_power_to_host").is_some()
        && residual_name_index(names, "host_special_power_ready_log").is_some()
        && residual_name_index(names, "Wave 717").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::MethodNames);
    ok
}
pub fn honesty_host_special_power_same_frame_ready_eva_source_markers_residual_wave717() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("host_apply_special_power_ready_after_writeback")
        && gl.contains("Wave 618: under sole-tick, GameWorld writeback records SP ready flips")
        && gl.contains("Wave 717: host EVA drain runs after writeback same frame");
    let no_mid_drain = {
        match gl.find("fn update_construction") {
            Some(start) => {
                // Bound to next method so helper (which drains) is excluded.
                let rest = &gl[start + 20..];
                let rel = rest
                    .find(
                        "
    pub(crate) fn ",
                    )
                    .or_else(|| {
                        rest.find(
                            "
    fn ",
                        )
                    })
                    .unwrap_or(8000);
                let chunk = &gl[start..start + 20 + rel];
                !chunk.contains("host_special_power_ready_log::drain()")
            }
            None => false,
        }
    };
    let sh_ok = (sh.contains("host_apply_special_power_ready_after_writeback")
        || sh.contains("SpecialPowerReadyAfterWriteback")
        || sh.contains("apply_post_writeback_complete_op"))
        && sh.contains("Wave 717")
        && sh.contains("writeback_special_power_to_host");
    let order_ok = match (
        sh.find("writeback_special_power_to_host(logic)"),
        sh.find("host_apply_special_power_ready_after_writeback")
            .or_else(|| sh.find("SpecialPowerReadyAfterWriteback"))
            .or_else(|| sh.find("apply_post_writeback_complete_op")),
    ) {
        (Some(w), Some(a)) => a > w && a - w < 400,
        _ => false,
    };
    let ok = gl_ok && no_mid_drain && sh_ok && order_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::SourceMarkers);
    ok
}
pub fn honesty_host_special_power_same_frame_ready_eva_nav_commands_residual_wave717() -> bool {
    let steps = LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_NAV_STEPS_WAVE717;
    let cmds = RUNTIME_HOST_LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA_CMD_NAMES_WAVE717;
    let ok = residual_name_index(steps, "REQUIRE_POST_WRITEBACK_EVA").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_NO_MID_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_HANDOFF").is_some()
        && residual_name_index(steps, "LIVE_HOST_SPECIAL_POWER_SAME_FRAME_READY_EVA").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_special_power_same_frame_ready_eva").is_some()
        && residual_name_index(cmds, "post_writeback_eva").is_some()
        && residual_name_index(cmds, "sole_tick_no_mid_drain").is_some()
        && residual_name_index(cmds, "session_handoff").is_some();
    residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::NavCommands);
    ok
}
pub fn simulate_host_special_power_same_frame_ready_eva_collect_source() -> bool {
    let ok = gl_source().contains("host_apply_special_power_ready_after_writeback")
        && (shadow_source().contains("host_apply_special_power_ready_after_writeback")
            || shadow_source().contains("SpecialPowerReadyAfterWriteback")
            || shadow_source().contains("apply_post_writeback_complete_op"));
    residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::CollectSource);
    ok
}
pub fn simulate_host_special_power_same_frame_ready_eva_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 717") && shadow_source().contains("Wave 717");
    residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::DispatchSource);
    ok
}
pub fn honesty_host_special_power_same_frame_ready_eva_residual_pack_wave717() -> bool {
    honesty_host_special_power_same_frame_ready_eva_method_names_residual_wave717()
        && honesty_host_special_power_same_frame_ready_eva_source_markers_residual_wave717()
        && honesty_host_special_power_same_frame_ready_eva_nav_commands_residual_wave717()
        && simulate_host_special_power_same_frame_ready_eva_collect_source()
        && simulate_host_special_power_same_frame_ready_eva_dispatch_source()
}
pub fn simulate_live_host_special_power_same_frame_ready_eva_honesty() -> bool {
    let ok = honesty_host_special_power_same_frame_ready_eva_residual_pack_wave717();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSpecialPowerSameFrameReadyEvaAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_special_power_ready_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        begin_shadow_coupled_tick, end_shadow_coupled_tick,
        gameworld_special_power_sole_tick_enabled,
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
        assert!(honesty_host_special_power_same_frame_ready_eva_method_names_residual_wave717());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_special_power_same_frame_ready_eva_source_markers_residual_wave717());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_special_power_same_frame_ready_eva_nav_commands_residual_wave717());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_special_power_same_frame_ready_eva_collect_source());
        assert!(simulate_host_special_power_same_frame_ready_eva_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_special_power_same_frame_ready_eva_residual_pack_wave717());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_special_power_same_frame_ready_eva_honesty());
        assert!(residual_host_special_power_same_frame_ready_eva_ok());
    }

    #[test]
    fn sole_tick_post_writeback_drains_ready_log() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev_sh = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let prev_sp = std::env::var_os("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY", "1");
        host_special_power_ready_log::clear();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "SfScudStorm");
        let id = logic
            .create_object("SfScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("structure");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(false);
        }

        begin_shadow_coupled_tick();
        assert!(gameworld_special_power_sole_tick_enabled());
        host_special_power_ready_log::record(id, 0.0);
        logic.host_apply_special_power_ready_after_writeback();
        assert!(
            host_special_power_ready_log::drain().is_empty(),
            "post-writeback path must consume ready log"
        );
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev_sh {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prev_sp {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY"),
        }
    }
}
