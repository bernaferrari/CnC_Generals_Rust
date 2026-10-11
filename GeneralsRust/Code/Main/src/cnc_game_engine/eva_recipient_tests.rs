//! Native delivery witnesses: no process EVA registry, test clock, or locks.
use super::*;
use crate::game_logic::host_eva_log::HostEvaAnnouncements;
use gamelogic::helpers::EvaEvent;

fn recipient() -> EvaRecipient {
    EvaRecipient {
        last_receipt: None,
        #[cfg(feature = "game_client")]
        speaker: game_client::eva::Eva::new(),
        #[cfg(not(feature = "game_client"))]
        enabled: true,
    }
}

#[test]
fn receipt_repetition_does_not_reapply_enable_and_clear_new_latches() {
    let mut owner = HostEvaAnnouncements::default();
    let mut native = recipient();
    owner.set_enabled(true);
    owner.request(EvaEvent::UnitLost);
    let delivery = owner.take_delivery();
    assert_eq!(native.admit(&delivery), EvaAdmission::NewSession);
    #[cfg(feature = "game_client")]
    {
        native
            .speaker
            .set_should_play(game_client::eva::EvaMessage::UpgradeComplete);
        assert_eq!(
            native.speaker.pending_messages(),
            vec![
                game_client::eva::EvaMessage::UnitLost,
                game_client::eva::EvaMessage::UpgradeComplete
            ]
        );
    }
    assert_eq!(native.admit(&delivery), EvaAdmission::Repeated);
    assert_eq!(native.admit(&owner.take_delivery()), EvaAdmission::Repeated);
    #[cfg(feature = "game_client")]
    assert_eq!(
        native.speaker.pending_messages(),
        vec![
            game_client::eva::EvaMessage::UnitLost,
            game_client::eva::EvaMessage::UpgradeComplete
        ]
    );
}

#[test]
fn ordered_controls_clear_prior_requests_and_admit_requests_while_disabled() {
    let mut owner = HostEvaAnnouncements::default();
    let mut native = recipient();
    owner.request(EvaEvent::BuildingLost);
    owner.set_enabled(true);
    owner.request(EvaEvent::UnitLost);
    let delivery = owner.take_delivery();
    assert_eq!(native.admit(&delivery), EvaAdmission::NewSession);
    #[cfg(feature = "game_client")]
    assert_eq!(
        native.speaker.pending_messages(),
        vec![game_client::eva::EvaMessage::UnitLost]
    );
    owner.set_enabled(false);
    owner.request(EvaEvent::GeneralLevelUp);
    assert_eq!(
        native.admit(&owner.take_delivery()),
        EvaAdmission::Continued
    );
    assert!(!native.is_enabled());
    #[cfg(feature = "game_client")]
    assert_eq!(
        native.speaker.pending_messages(),
        vec![game_client::eva::EvaMessage::GeneralLevelUp]
    );
    owner.set_enabled(true);
    owner.set_enabled(true);
    owner.request(EvaEvent::UpgradeComplete);
    assert_eq!(
        native.admit(&owner.take_delivery()),
        EvaAdmission::Continued
    );
    assert!(native.is_enabled());
    #[cfg(feature = "game_client")]
    assert_eq!(
        native.speaker.pending_messages(),
        vec![game_client::eva::EvaMessage::UpgradeComplete]
    );
}

#[test]
fn different_worlds_and_reset_are_new_sessions_at_identical_cursors() {
    let mut a = HostEvaAnnouncements::default();
    let mut b = HostEvaAnnouncements::default();
    let mut native_a = recipient();
    let mut native_b = recipient();
    a.request(EvaEvent::UnitLost);
    b.request(EvaEvent::BuildingLost);
    let batch_a = a.take_delivery();
    let batch_b = b.take_delivery();
    assert_eq!(batch_a.receipt.cursor, batch_b.receipt.cursor);
    assert!(!batch_a.receipt.same_session(&batch_b.receipt));
    assert_eq!(native_a.admit(&batch_a), EvaAdmission::NewSession);
    assert_eq!(native_b.admit(&batch_b), EvaAdmission::NewSession);
    #[cfg(feature = "game_client")]
    {
        assert_eq!(
            native_a.speaker.pending_messages(),
            vec![game_client::eva::EvaMessage::UnitLost]
        );
        assert_eq!(
            native_b.speaker.pending_messages(),
            vec![game_client::eva::EvaMessage::BuildingLost]
        );
    }
    a.reset();
    a.request(EvaEvent::CashStolen);
    let restarted = a.take_delivery();
    assert_eq!(restarted.receipt.cursor, batch_a.receipt.cursor);
    assert_eq!(native_a.admit(&restarted), EvaAdmission::NewSession);
    drop(a);
    drop(b);
    #[cfg(feature = "game_client")]
    {
        assert_eq!(
            native_a.speaker.pending_messages(),
            vec![game_client::eva::EvaMessage::CashStolen]
        );
        assert_eq!(
            native_b.speaker.pending_messages(),
            vec![game_client::eva::EvaMessage::BuildingLost]
        );
    }
}

#[test]
fn recipient_attached_after_consumption_observes_enabled_baseline() {
    let mut owner = HostEvaAnnouncements::default();
    owner.set_enabled(false);
    let consumed = owner.take_delivery();
    assert!(!consumed.operations.is_empty());
    let empty = owner.take_delivery();
    assert!(empty.operations.is_empty());
    let mut native = recipient();
    assert_eq!(native.admit(&empty), EvaAdmission::NewSession);
    assert!(!native.is_enabled());
    assert_eq!(native.admit(&empty), EvaAdmission::Repeated);
    owner.set_enabled(true);
    assert_eq!(
        native.admit(&owner.take_delivery()),
        EvaAdmission::Continued
    );
    assert!(native.is_enabled());
}

#[test]
fn cached_older_receipt_cannot_replay_controls_or_resurrect_cleared_requests() {
    let mut owner = HostEvaAnnouncements::default();
    let mut native = recipient();
    owner.set_enabled(false);
    owner.request(EvaEvent::UnitLost);
    let old = owner.take_delivery();
    assert_eq!(native.admit(&old), EvaAdmission::NewSession);
    owner.set_enabled(true);
    owner.request(EvaEvent::UpgradeComplete);
    let latest = owner.take_delivery();
    assert_eq!(native.admit(&latest), EvaAdmission::Continued);
    assert!(native.is_enabled());
    assert_eq!(native.admit(&old), EvaAdmission::Repeated);
    assert!(native.is_enabled());
    #[cfg(feature = "game_client")]
    assert_eq!(
        native.speaker.pending_messages(),
        vec![game_client::eva::EvaMessage::UpgradeComplete]
    );
}

#[test]
fn cleared_and_disabled_requests_are_absorbed_before_counter_fallback() {
    let mut owner = HostEvaAnnouncements::default();
    owner.request(EvaEvent::LowPower);
    owner.set_enabled(true);
    owner.request(EvaEvent::InsufficientFunds);
    owner.set_enabled(false);
    owner.request(EvaEvent::BaseUnderAttack);
    owner.set_enabled(true);
    let delivery = owner.take_delivery();
    let mut native = recipient();
    assert_eq!(native.admit(&delivery), EvaAdmission::NewSession);
    #[cfg(feature = "game_client")]
    assert!(native.speaker.pending_messages().is_empty());
    let counters = [3, 4, 5, 0];
    assert_eq!(
        absorb_request_counters(&delivery, [0; 4], counters),
        counters,
        "counter diagnostics must not revive these canceled HUD warnings"
    );
    let empty = owner.take_delivery();
    assert_eq!(
        absorb_request_counters(&empty, counters, [3, 4, 5, 7]),
        counters,
        "only actually observed intents suppress the counter-only fallback"
    );
}
