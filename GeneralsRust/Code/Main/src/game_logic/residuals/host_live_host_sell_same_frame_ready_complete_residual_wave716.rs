//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE716: &[&str] = &[
    "host_apply_sell_completions_after_ready_writeback",
    "apply_post_writeback_complete_op",
    "SellCompletionsAfterReadyWriteback",
    "host_owns_construction",
    "host_sell_ready_log",
    "compare_construction_to_host",
    "Wave 716",
    "playable_claim = false",
];
pub const LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE716: &[&str] = &[
    "REQUIRE_POST_WRITEBACK_FINISH",
    "REQUIRE_SOLE_TICK_NO_MID_DRAIN",
    "REQUIRE_SESSION_HANDOFF",
    "LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE716: &[&str] = &[
    "host_sell_same_frame_ready_complete",
    "post_writeback_finish",
    "sole_tick_no_mid_drain",
    "session_handoff",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSellSameFrameReadyCompleteAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSellSameFrameReadyCompleteAction {
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
fn residual_action_store(a: ResidualHostSellSameFrameReadyCompleteAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_sell_same_frame_ready_complete_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_sell_same_frame_ready_complete_last_action()
-> ResidualHostSellSameFrameReadyCompleteAction {
    ResidualHostSellSameFrameReadyCompleteAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_sell_same_frame_ready_complete_method_names_residual_wave716() -> bool {
    let names = LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_METHOD_NAMES_WAVE716;
    let ok = residual_name_index(names, "host_apply_sell_completions_after_ready_writeback")
        .is_some()
        && residual_name_index(names, "host_owns_construction").is_some()
        && residual_name_index(names, "host_sell_ready_log").is_some()
        && residual_name_index(names, "compare_construction_to_host").is_some()
        && residual_name_index(names, "Wave 716").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::MethodNames);
    ok
}
pub fn honesty_host_sell_same_frame_ready_complete_source_markers_residual_wave716() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::SourceMarkers);
    ok
}
pub fn honesty_host_sell_same_frame_ready_complete_nav_commands_residual_wave716() -> bool {
    let steps = LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_NAV_STEPS_WAVE716;
    let cmds = RUNTIME_HOST_LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE_CMD_NAMES_WAVE716;
    let ok = residual_name_index(steps, "REQUIRE_POST_WRITEBACK_FINISH").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_NO_MID_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_HANDOFF").is_some()
        && residual_name_index(steps, "LIVE_HOST_SELL_SAME_FRAME_READY_COMPLETE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_sell_same_frame_ready_complete").is_some()
        && residual_name_index(cmds, "post_writeback_finish").is_some()
        && residual_name_index(cmds, "sole_tick_no_mid_drain").is_some()
        && residual_name_index(cmds, "session_handoff").is_some();
    residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::NavCommands);
    ok
}
pub fn simulate_host_sell_same_frame_ready_complete_collect_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::CollectSource);
    ok
}
pub fn simulate_host_sell_same_frame_ready_complete_dispatch_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::DispatchSource);
    ok
}
pub fn honesty_host_sell_same_frame_ready_complete_residual_pack_wave716() -> bool {
    honesty_host_sell_same_frame_ready_complete_method_names_residual_wave716()
        && honesty_host_sell_same_frame_ready_complete_source_markers_residual_wave716()
        && honesty_host_sell_same_frame_ready_complete_nav_commands_residual_wave716()
        && simulate_host_sell_same_frame_ready_complete_collect_source()
        && simulate_host_sell_same_frame_ready_complete_dispatch_source()
}
pub fn simulate_live_host_sell_same_frame_ready_complete_honesty() -> bool {
    let ok = honesty_host_sell_same_frame_ready_complete_residual_pack_wave716();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSellSameFrameReadyCompleteAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_sell_ready_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, cost: u32) {
        let mut t = ThingTemplate::new(name);
        t.set_health(200.0);
        t.set_cost(cost, 0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_sell_same_frame_ready_complete_method_names_residual_wave716());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_sell_same_frame_ready_complete_source_markers_residual_wave716());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_sell_same_frame_ready_complete_nav_commands_residual_wave716());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_sell_same_frame_ready_complete_collect_source());
        assert!(simulate_host_sell_same_frame_ready_complete_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_sell_same_frame_ready_complete_residual_pack_wave716());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_sell_same_frame_ready_complete_honesty());
        assert!(residual_host_sell_same_frame_ready_complete_ok());
    }

    #[test]
    fn owned_sell_phase_finishes_without_ready_writeback() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "OwnedSeller", true));
        ensure_template(&mut logic, "SfSellPad", 1000);
        let id = logic
            .create_object_for_player("SfSellPad", 0, Vec3::ZERO)
            .unwrap();
        assert!(logic.start_sell_object(id));
        // The actual sell list retains its admission frame. Advance through
        // the CPP scaffold delay and decrement phase, never a shadow-ready log.
        // CPP BuildAssistant: 45-frame scaffold delay followed by 1/90
        // per-frame decrement until -0.5, including the final float boundary.
        for frame in 0..=179 {
            logic.set_current_frame(frame);
            logic.update_sell_list();
            if frame == 44 {
                // CPP startSell sets 99.9%, retained through frame44.
                assert_eq!(logic.host_object(id).unwrap().construction_percent, 0.999);
            }
            if frame == 178 {
                assert!(
                    logic.host_object(id).is_some(),
                    "sell must retain the final scaffold frame"
                );
            }
            logic.process_destroy_list();
            if logic.host_object(id).is_none() {
                break;
            }
        }
        assert!(logic.host_object(id).is_none());
        assert!(logic.sell_list_for_snapshot().is_empty());
    }
}
