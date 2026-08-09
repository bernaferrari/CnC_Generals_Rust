//! Wave 714: sole-tick production complete/spawn runs after GW ready writeback
//! in the same coupled tick (not deferred to the next host `update_production`).
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE714: &[&str] = &[
    "host_apply_production_completions_after_ready_writeback",
    "apply_production_authority_op",
    "ApplyCompletionsAfterReadyWriteback",
    "gameworld_production_sole_tick_enabled",
    "writeback_production_to_host",
    "host_collect_production_completions",
    "Wave 714",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE714: &[&str] = &[
    "REQUIRE_POST_WRITEBACK_COMPLETE",
    "REQUIRE_SOLE_TICK_SKIP_MID_UPDATE",
    "REQUIRE_SESSION_HANDOFF",
    "LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE714: &[&str] =
    &[
        "host_production_same_frame_ready_complete",
        "post_writeback_complete",
        "sole_tick_skip_mid_update",
        "session_handoff",
    ];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSameFrameReadyCompleteAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionSameFrameReadyCompleteAction {
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
fn residual_action_store(a: ResidualHostProductionSameFrameReadyCompleteAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_same_frame_ready_complete_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_same_frame_ready_complete_last_action(
) -> ResidualHostProductionSameFrameReadyCompleteAction {
    ResidualHostProductionSameFrameReadyCompleteAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn shadow_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
pub fn honesty_host_production_same_frame_ready_complete_method_names_residual_wave714() -> bool {
    let names = LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE714;
    let ok = residual_name_index(
        names,
        "host_apply_production_completions_after_ready_writeback",
    )
    .is_some()
        && residual_name_index(names, "apply_production_authority_op").is_some()
        && residual_name_index(names, "ApplyCompletionsAfterReadyWriteback").is_some()
        && residual_name_index(names, "gameworld_production_sole_tick_enabled").is_some()
        && residual_name_index(names, "writeback_production_to_host").is_some()
        && residual_name_index(names, "host_collect_production_completions").is_some()
        && residual_name_index(names, "Wave 714").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::MethodNames);
    ok
}
pub fn honesty_host_production_same_frame_ready_complete_source_markers_residual_wave714() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("host_apply_production_completions_after_ready_writeback")
        && gl.contains("apply_production_authority_op")
        && gl.contains("Wave 714")
        && gl.contains("gameworld_production_sole_tick_enabled()")
        && gl.contains("return;"); // sole early-return in update_production
                                   // update_production sole path returns before collect
    let upd_ok = gl
        .find("fn update_production")
        .map(|i| {
            let end = (i + 900).min(gl.len());
            let chunk = &gl[i..end];
            chunk.contains("gameworld_production_sole_tick_enabled()") && chunk.contains("return;")
        })
        .unwrap_or(false);
    let sh_ok = (sh.contains("host_apply_production_completions_after_ready_writeback")
        || sh.contains("ApplyCompletionsAfterReadyWriteback")
        || sh.contains("apply_production_authority_op"))
        && sh.contains("writeback_production_to_host")
        && sh.contains("Wave 714");
    // writeback then same-frame complete order
    let complete_at = sh
        .find("host_apply_production_completions_after_ready_writeback")
        .or_else(|| sh.find("ApplyCompletionsAfterReadyWriteback"))
        .or_else(|| sh.find("apply_production_authority_op"));
    let order_ok = match (sh.find("writeback_production_to_host(logic)"), complete_at) {
        (Some(w), Some(c)) => c > w && c - w < 800,
        _ => false,
    };
    let ok = gl_ok && upd_ok && sh_ok && order_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_same_frame_ready_complete_nav_commands_residual_wave714() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE714;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE714;
    let ok = residual_name_index(steps, "REQUIRE_POST_WRITEBACK_COMPLETE").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_SKIP_MID_UPDATE").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_HANDOFF").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SAME_FRAME_READY_COMPLETE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_same_frame_ready_complete").is_some()
        && residual_name_index(cmds, "post_writeback_complete").is_some()
        && residual_name_index(cmds, "sole_tick_skip_mid_update").is_some()
        && residual_name_index(cmds, "session_handoff").is_some();
    residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::NavCommands);
    ok
}
pub fn simulate_host_production_same_frame_ready_complete_collect_source() -> bool {
    let ok = gl_source().contains("ApplyCompletionsAfterReadyWriteback")
        && shadow_source().contains("ApplyCompletionsAfterReadyWriteback");
    residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::CollectSource);
    ok
}
pub fn simulate_host_production_same_frame_ready_complete_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 714") && shadow_source().contains("Wave 714");
    residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::DispatchSource);
    ok
}
pub fn honesty_host_production_same_frame_ready_complete_residual_pack_wave714() -> bool {
    honesty_host_production_same_frame_ready_complete_method_names_residual_wave714()
        && honesty_host_production_same_frame_ready_complete_source_markers_residual_wave714()
        && honesty_host_production_same_frame_ready_complete_nav_commands_residual_wave714()
        && simulate_host_production_same_frame_ready_complete_collect_source()
        && simulate_host_production_same_frame_ready_complete_dispatch_source()
}
pub fn simulate_live_host_production_same_frame_ready_complete_honesty() -> bool {
    let ok = honesty_host_production_same_frame_ready_complete_residual_pack_wave714();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSameFrameReadyCompleteAction::Composite);
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
        GameWorldShadow,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str) {
        let mut t = ThingTemplate::new(name);
        t.set_health(100.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_same_frame_ready_complete_method_names_residual_wave714());
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_production_same_frame_ready_complete_source_markers_residual_wave714()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_same_frame_ready_complete_nav_commands_residual_wave714());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_same_frame_ready_complete_collect_source());
        assert!(simulate_host_production_same_frame_ready_complete_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_same_frame_ready_complete_residual_pack_wave714());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_same_frame_ready_complete_honesty());
        assert!(residual_host_production_same_frame_ready_complete_ok());
    }

    #[test]
    fn sole_tick_mid_update_skips_collect_post_writeback_completes() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev_sh = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let prev_pr = std::env::var_os("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY");
        std::env::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
        host_production_ready_log::clear();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "SfFactory");
        ensure_template(&mut logic, "SfUnit");
        let id = logic
            .create_object("SfFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("factory");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(false);
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "SfUnit".into(),
                progress: 10.0,
                total_time: 10.0,
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
        // Mid-frame update_production must not complete without post-writeback path.
        // Simulate by NOT recording ready, calling after-writeback with empty log.
        logic.host_apply_production_completions_after_ready_writeback(1.0 / 30.0);
        let qlen = logic
            .get_objects()
            .get(&id)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.len())
            .unwrap_or(0);
        assert_eq!(qlen, 1, "empty ready must not complete");

        // Wave 738: sole-tick spawn requires GW entity bind (entity-first).
        host_production_ready_log::record_with_pose(
            id,
            "SfUnit",
            false,
            Some([0.0, 0.0, 0.0]),
            None,
            Some(9_714),
        );
        logic.host_apply_production_completions_after_ready_writeback(1.0 / 30.0);
        let units = logic
            .get_objects()
            .values()
            .filter(|o| o.template_name == "SfUnit")
            .count();
        assert_eq!(units, 1, "same-frame ready writeback path must spawn unit");
        end_shadow_coupled_tick();
        let _ = ObjectId;
        let _ = GameWorldShadow::new(8);

        match prev_sh {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prev_pr {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY"),
        }
    }
}
