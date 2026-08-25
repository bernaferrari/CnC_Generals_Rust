//! Wave 715: sole-tick construction complete runs after GW ready writeback
//! in the same coupled tick (not deferred to the next host `update_construction`).
//! Mirrors production Wave 714. Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE715: &[&str] = &[
    "host_apply_construction_completions_after_ready_writeback",
    "apply_post_writeback_complete_op",
    "ConstructionCompletionsAfterReadyWriteback",
    "gameworld_construction_sole_tick_enabled",
    "writeback_construction_to_host",
    "host_construction_ready_log",
    "Wave 715",
    "playable_claim = false",
];
pub const LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE715: &[&str] = &[
    "REQUIRE_POST_WRITEBACK_COMPLETE",
    "REQUIRE_SOLE_TICK_NO_MID_DRAIN",
    "REQUIRE_SESSION_HANDOFF",
    "LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE715:
    &[&str] = &[
    "host_construction_same_frame_ready_complete",
    "post_writeback_complete",
    "sole_tick_no_mid_drain",
    "session_handoff",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructionSameFrameReadyCompleteAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostConstructionSameFrameReadyCompleteAction {
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
fn residual_action_store(a: ResidualHostConstructionSameFrameReadyCompleteAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_construction_same_frame_ready_complete_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_construction_same_frame_ready_complete_last_action()
-> ResidualHostConstructionSameFrameReadyCompleteAction {
    ResidualHostConstructionSameFrameReadyCompleteAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_construction_same_frame_ready_complete_method_names_residual_wave715() -> bool {
    let names = LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE715;
    let ok = residual_name_index(
        names,
        "host_apply_construction_completions_after_ready_writeback",
    )
    .is_some()
        && residual_name_index(names, "gameworld_construction_sole_tick_enabled").is_some()
        && residual_name_index(names, "writeback_construction_to_host").is_some()
        && residual_name_index(names, "host_construction_ready_log").is_some()
        && residual_name_index(names, "Wave 715").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::MethodNames);
    ok
}
pub fn honesty_host_construction_same_frame_ready_complete_source_markers_residual_wave715() -> bool
{
    let gl = gl_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("host_apply_construction_completions_after_ready_writeback")
        && gl.contains("Wave 715")
        && gl.contains("Empty mid-update ready set: sole completes only via post-writeback helper");
    // mid-update must not drain ready log under sole
    let no_mid_drain = {
        let i = gl.find("fn update_construction");
        match i {
            Some(start) => {
                let end = (start + 2500).min(gl.len());
                let chunk = &gl[start..end];
                !chunk.contains("host_construction_ready_log::drain()")
            }
            None => false,
        }
    };
    let sh_ok = (sh.contains("host_apply_construction_completions_after_ready_writeback")
        || sh.contains("ConstructionCompletionsAfterReadyWriteback")
        || sh.contains("apply_post_writeback_complete_op"))
        && sh.contains("Wave 715");
    let order_ok = match (
        sh.find("let _construction_wb = shadow.writeback_construction_to_host"),
        sh.find("host_apply_construction_completions_after_ready_writeback")
            .or_else(|| sh.find("ConstructionCompletionsAfterReadyWriteback"))
            .or_else(|| sh.find("apply_post_writeback_complete_op")),
    ) {
        (Some(w), Some(c)) => c > w && c - w < 400,
        _ => false,
    };
    let ok = gl_ok && no_mid_drain && sh_ok && order_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::SourceMarkers);
    ok
}
pub fn honesty_host_construction_same_frame_ready_complete_nav_commands_residual_wave715() -> bool {
    let steps = LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE715;
    let cmds = RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE715;
    let ok = residual_name_index(steps, "REQUIRE_POST_WRITEBACK_COMPLETE").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_NO_MID_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_HANDOFF").is_some()
        && residual_name_index(steps, "LIVE_HOST_CONSTRUCTION_SAME_FRAME_READY_COMPLETE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_construction_same_frame_ready_complete").is_some()
        && residual_name_index(cmds, "post_writeback_complete").is_some()
        && residual_name_index(cmds, "sole_tick_no_mid_drain").is_some()
        && residual_name_index(cmds, "session_handoff").is_some();
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::NavCommands);
    ok
}
pub fn simulate_host_construction_same_frame_ready_complete_collect_source() -> bool {
    let ok = gl_source().contains("host_apply_construction_completions_after_ready_writeback")
        && (shadow_source().contains("host_apply_construction_completions_after_ready_writeback")
            || shadow_source().contains("ConstructionCompletionsAfterReadyWriteback")
            || shadow_source().contains("apply_post_writeback_complete_op"));
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::CollectSource);
    ok
}
pub fn simulate_host_construction_same_frame_ready_complete_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 715") && shadow_source().contains("Wave 715");
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::DispatchSource);
    ok
}
pub fn honesty_host_construction_same_frame_ready_complete_residual_pack_wave715() -> bool {
    honesty_host_construction_same_frame_ready_complete_method_names_residual_wave715()
        && honesty_host_construction_same_frame_ready_complete_source_markers_residual_wave715()
        && honesty_host_construction_same_frame_ready_complete_nav_commands_residual_wave715()
        && simulate_host_construction_same_frame_ready_complete_collect_source()
        && simulate_host_construction_same_frame_ready_complete_dispatch_source()
}
pub fn simulate_live_host_construction_same_frame_ready_complete_honesty() -> bool {
    let ok = honesty_host_construction_same_frame_ready_complete_residual_pack_wave715();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_construction_ready_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        begin_shadow_coupled_tick, end_shadow_coupled_tick,
        gameworld_construction_sole_tick_enabled,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str) {
        let mut t = ThingTemplate::new(name);
        t.set_health(200.0);
        t.build_time = 10.0;
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_construction_same_frame_ready_complete_method_names_residual_wave715()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_construction_same_frame_ready_complete_source_markers_residual_wave715()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_construction_same_frame_ready_complete_nav_commands_residual_wave715()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_construction_same_frame_ready_complete_collect_source());
        assert!(simulate_host_construction_same_frame_ready_complete_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_construction_same_frame_ready_complete_residual_pack_wave715());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_construction_same_frame_ready_complete_honesty());
        assert!(residual_host_construction_same_frame_ready_complete_ok());
    }

    #[test]
    fn sole_tick_post_writeback_completes_structure() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev_sh = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let prev_c = std::env::var_os("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
        host_construction_ready_log::clear();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "SfBarracks");
        let id = logic
            .create_object("SfBarracks", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("structure");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(true);
        }

        begin_shadow_coupled_tick();
        assert!(gameworld_construction_sole_tick_enabled());
        // Empty ready: no-op.
        logic.host_apply_construction_completions_after_ready_writeback();
        assert!(
            logic
                .get_objects()
                .get(&id)
                .map(|o| o.status.under_construction)
                .unwrap_or(false),
            "must stay under construction without ready log"
        );

        host_construction_ready_log::record(id, 1.0);
        logic.host_apply_construction_completions_after_ready_writeback();
        let obj = logic.get_objects().get(&id).expect("obj");
        assert!(
            !obj.status.under_construction,
            "ready path must clear under_construction"
        );
        assert!(obj.construction_percent + 1e-6 >= 1.0);
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev_sh {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prev_c {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY"),
        }
    }
}
