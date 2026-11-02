//! Eva lives on InGameUISubsystem (no process TLS).

use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::eva::{Eva, EvaMessage};

#[test]
fn eva_defaults_on_in_game_ui_subsystem() {
    let ui = InGameUISubsystem::default();
    assert!(ui.eva.is_enabled());
    assert_eq!(ui.eva.host_frame(), 0);
    assert!(ui.eva.pending_messages().is_empty());
}

#[test]
fn eva_host_frame_is_per_speaker() {
    let mut a = Eva::new();
    let mut b = Eva::new();
    a.set_host_frame(5);
    assert_eq!(a.logic_frame(), 5);
    assert_eq!(b.host_frame(), 0);
    a.set_should_play(EvaMessage::UnitLost);
    assert_eq!(a.pending_messages(), vec![EvaMessage::UnitLost]);
    assert!(b.pending_messages().is_empty());
}
