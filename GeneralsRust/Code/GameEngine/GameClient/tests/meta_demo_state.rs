//! MetaDemoState lives on InGameUISubsystem (no process TLS).

use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::meta_demo_state::MetaDemoState;

#[test]
fn meta_demo_state_defaults_on_in_game_ui_subsystem() {
    let ui = InGameUISubsystem::default();
    assert!(!ui.meta_demo.vtune_enabled);
    assert!(!ui.meta_demo.hand_of_god_mode);
    assert_eq!(ui.meta_demo.bw_view_mode, 0);
    assert!((ui.meta_demo.skate_distance_override - 0.0).abs() < f32::EPSILON);
}

#[test]
fn meta_demo_toggle_bool_flips_owned_flag() {
    let mut demo = MetaDemoState::default();
    assert!(!demo.hurt_me_mode);
    assert!(MetaDemoState::toggle_bool(&mut demo.hurt_me_mode));
    assert!(demo.hurt_me_mode);
    assert!(!MetaDemoState::toggle_bool(&mut demo.hurt_me_mode));
}
