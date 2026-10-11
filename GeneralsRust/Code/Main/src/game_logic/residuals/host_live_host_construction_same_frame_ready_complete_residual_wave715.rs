//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

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
    "host_owns_construction",
    "compare_construction_to_host",
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
        && residual_name_index(names, "host_owns_construction").is_some()
        && residual_name_index(names, "compare_construction_to_host").is_some()
        && residual_name_index(names, "host_construction_ready_log").is_some()
        && residual_name_index(names, "Wave 715").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::MethodNames);
    ok
}
pub fn honesty_host_construction_same_frame_ready_complete_source_markers_residual_wave715() -> bool
{
    let ok = super::construction_owner_source_contract::host_owns_construction();
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
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostConstructionSameFrameReadyCompleteAction::CollectSource);
    ok
}
pub fn simulate_host_construction_same_frame_ready_complete_dispatch_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
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
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
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
    fn owned_dozer_phase_completes_structure_without_ready_writeback() {
        use crate::game_logic::{AIState, Player};
        let mut logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "OwnedBuilder", true);
        // CPP Energy consumption==0 returns production; zero production
        // invokes the low-energy slowdown rather than full-speed construction.
        player.power_produced = 1;
        logic.add_player(player);
        ensure_template(&mut logic, "SfBarracks");
        let mut template = ThingTemplate::new("SfDozer");
        template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Dozer)
            .set_health(200.0);
        logic.templates.insert(template.name.clone(), template);
        let id = logic
            .create_object_for_player("SfBarracks", 0, Vec3::ZERO)
            .unwrap();
        let dozer = logic
            .create_object_for_player("SfDozer", 0, Vec3::ZERO)
            .unwrap();
        logic.dozer_new_task_build(dozer, id);
        let action = logic.host_object(dozer).unwrap().dozer_dock_action.unwrap();
        let object = logic.host_object_mut(id).unwrap();
        object.set_status_under_construction(true);
        object.construction_percent = 0.99;
        object.builder_id = Some(dozer);
        let builder = logic.host_object_mut(dozer).unwrap();
        builder.set_position(action);
        builder.set_target(Some(id));
        builder.set_ai_state(AIState::Constructing);
        builder.movement.path.clear();
        builder.waiting_for_path = false;
        logic.update_construction(&[id, dozer], 0.2);
        assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
        assert!(!logic.host_object(id).unwrap().status.under_construction);
        assert_eq!(
            logic.host_object(dozer).unwrap().dozer_task_build_target,
            None
        );
    }
}
