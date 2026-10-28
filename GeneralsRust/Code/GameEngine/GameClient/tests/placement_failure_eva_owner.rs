//! Placement confirmation emits both UI and EVA feedback to the receiving owner.
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::eva::{Eva, EvaMessage};
use game_client_rust::message_stream::{ReceivingUi, play_can_make_failure_with_eva};
use game_engine::common::system::build_assistant::CanMakeType;

#[test]
fn insufficient_funds_updates_receiving_hud_and_only_receiving_eva() {
    let mut ui = InGameUISubsystem::default();
    let mut receiving = Eva::new();
    let peer = Eva::new();

    {
        let mut receiving_ui = ReceivingUi::new(&mut ui, 100, 100);
        play_can_make_failure_with_eva(CanMakeType::NoMoney, &mut receiving_ui, &mut receiving);
    }

    assert_eq!(ui.drain_hud_messages(), ["GUI:NotEnoughMoneyToBuild"]);
    assert_eq!(
        receiving.pending_messages(),
        vec![EvaMessage::InsufficientFunds]
    );
    assert!(peer.pending_messages().is_empty());
}
