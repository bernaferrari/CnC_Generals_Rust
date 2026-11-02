//! Observable ownership for ControlBar residual peels (multi-select, resizer,
//! under-construction, OCL timer).
//!
//! Each C++ ControlBar* residual kept UI state on the bar instance; Rust latches
//! are per-client so two peels never share process atomics or TLS.

use game_client_rust::gui::control_bar::{
    ControlBarResizerLatch, MultiSelectLatch, OclTimerLatch, ResidualControlBarResizerAction,
    ResidualMultiSelectAction, ResidualOclTimerAction, ResidualUnderConstructionAction,
    UnderConstructionLatch, residual_control_bar_resizer_base_resolution,
    residual_control_bar_resizer_last_action, residual_control_bar_resizer_window_count,
    residual_multi_select_attack_move_kept, residual_multi_select_common_command_count,
    residual_multi_select_last_action, residual_multi_select_portrait_agrees,
    residual_ocl_timer_last_action, residual_ocl_timer_seconds,
    residual_under_construction_cancel_visible, residual_under_construction_last_action,
    residual_under_construction_percent, simulate_control_bar_resizer_prepare_default,
    simulate_multi_select_clear, simulate_multi_select_prepare_divergent,
    simulate_multi_select_prepare_same_commands, simulate_ocl_timer_prepare_display,
    simulate_under_construction_complete, simulate_under_construction_prepare_cycle,
};

#[test]
fn multi_select_latch_is_instance_owned() {
    let mut a = MultiSelectLatch::default();
    let mut b = MultiSelectLatch::default();
    assert!(simulate_multi_select_prepare_same_commands(&mut a));
    assert_eq!(residual_multi_select_common_command_count(&a), 3);
    assert!(residual_multi_select_portrait_agrees(&a));
    assert!(residual_multi_select_attack_move_kept(&a));
    assert_eq!(residual_multi_select_common_command_count(&b), 0);
    assert!(simulate_multi_select_prepare_divergent(&mut b));
    assert_eq!(
        residual_multi_select_last_action(&b),
        ResidualMultiSelectAction::Intersect
    );
    assert_eq!(residual_multi_select_common_command_count(&b), 1);
    assert_eq!(residual_multi_select_common_command_count(&a), 3);
    assert!(simulate_multi_select_clear(&mut a));
    assert_eq!(
        residual_multi_select_last_action(&a),
        ResidualMultiSelectAction::Clear
    );
}

#[test]
fn control_bar_resizer_latch_is_instance_owned() {
    let mut a = ControlBarResizerLatch::default();
    let b = ControlBarResizerLatch::default();
    assert!(simulate_control_bar_resizer_prepare_default(&mut a));
    assert_eq!(residual_control_bar_resizer_window_count(&a), 1);
    assert_eq!(residual_control_bar_resizer_base_resolution(&a), (800, 600));
    assert_eq!(
        residual_control_bar_resizer_last_action(&a),
        ResidualControlBarResizerAction::Prepare
    );
    assert_eq!(residual_control_bar_resizer_window_count(&b), 0);
    assert_eq!(
        residual_control_bar_resizer_last_action(&b),
        ResidualControlBarResizerAction::None
    );
}

#[test]
fn under_construction_latch_is_instance_owned() {
    let mut a = UnderConstructionLatch::default();
    let b = UnderConstructionLatch::default();
    assert!(simulate_under_construction_prepare_cycle(
        &mut a, "Strategy Center", 40, 55
    ));
    assert_eq!(residual_under_construction_percent(&a), 55);
    assert!(residual_under_construction_cancel_visible(&a));
    assert_eq!(residual_under_construction_percent(&b), -1);
    assert!(simulate_under_construction_complete(&mut a));
    assert_eq!(
        residual_under_construction_last_action(&a),
        ResidualUnderConstructionAction::Complete
    );
    assert!(!residual_under_construction_cancel_visible(&a));
    assert_eq!(
        residual_under_construction_last_action(&b),
        ResidualUnderConstructionAction::None
    );
}

#[test]
fn ocl_timer_latch_is_instance_owned() {
    let mut a = OclTimerLatch::default();
    let b = OclTimerLatch::default();
    let prepared = simulate_ocl_timer_prepare_display(&mut a, 450, 900);
    assert!(prepared.is_some());
    assert_eq!(residual_ocl_timer_seconds(&a), 15);
    assert_eq!(
        residual_ocl_timer_last_action(&a),
        ResidualOclTimerAction::Prepare
    );
    assert_eq!(residual_ocl_timer_seconds(&b), 0);
    assert_eq!(
        residual_ocl_timer_last_action(&b),
        ResidualOclTimerAction::None
    );
}
