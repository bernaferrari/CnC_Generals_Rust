//! Same-thread intent ownership and reset receipt witnesses.
use super::*;
use crate::game_logic::GameLogic;

#[test]
fn eva_announcement_owner_tests_world_lifecycles_do_not_consume_foreign_intent() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    assert_eq!(a.get_frame(), b.get_frame());
    a.eva_announcements.request(EvaEvent::UpgradeComplete);
    b.eva_announcements.request(EvaEvent::GeneralLevelUp);
    b.set_eva_enabled(false);
    b.eva_announcements.reset();
    drop(b);
    let receipt = a.eva_announcements.take_delivery();
    assert_eq!(
        receipt.operations,
        vec![EvaOperation::Request(EvaEvent::UpgradeComplete)]
    );
    assert!(receipt.enabled);
    let repeated = a.eva_announcements.take_delivery();
    assert_eq!(repeated.receipt, receipt.receipt);
    assert!(repeated.operations.is_empty());
    a.eva_announcements.reset();
    a.eva_announcements.request(EvaEvent::UpgradeComplete);
    let reset = a.eva_announcements.take_delivery();
    assert!(!reset.receipt.same_session(&receipt.receipt));
    assert_eq!(reset.receipt.cursor, receipt.receipt.cursor);
    assert_eq!(reset.operations, receipt.operations);
}

#[test]
fn eva_announcement_owner_tests_controls_and_disabled_requests_retain_original_order() {
    let mut owner = HostEvaAnnouncements::default();
    owner.request(EvaEvent::GeneralLevelUp);
    owner.set_enabled(true);
    owner.request(EvaEvent::UpgradeComplete);
    owner.set_enabled(false);
    owner.request(EvaEvent::BuildingSabotaged);
    owner.set_enabled(false);
    let batch = owner.take_delivery();
    assert_eq!(
        batch.operations,
        vec![
            EvaOperation::Request(EvaEvent::GeneralLevelUp),
            EvaOperation::SetEnabled(true),
            EvaOperation::Request(EvaEvent::UpgradeComplete),
            EvaOperation::SetEnabled(false),
            EvaOperation::Request(EvaEvent::BuildingSabotaged),
            EvaOperation::SetEnabled(false)
        ]
    );
    assert!(!batch.enabled);
}

#[test]
fn eva_announcement_owner_tests_rejected_detached_load_keeps_live_receipt() {
    let mut live = GameLogic::new();
    live.eva_announcements.request(EvaEvent::UpgradeComplete);
    let initial = live.eva_announcements.take_delivery();
    let mut candidate = GameLogic::new();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut malformed = builder
        .create_world_snapshot(&candidate)
        .expect("empty candidate snapshot");
    malformed.version = u32::MAX;
    assert!(
        builder
            .restore_from_snapshot(&malformed, &mut candidate)
            .is_err()
    );
    candidate.eva_announcements.reset();
    drop(candidate);
    let unchanged = live.eva_announcements.take_delivery();
    assert_eq!(unchanged.receipt, initial.receipt);
    assert!(unchanged.operations.is_empty());
    assert!(unchanged.enabled);
    live.eva_announcements.request(EvaEvent::GeneralLevelUp);
    assert_eq!(live.take_eva_requests(), vec![EvaEvent::GeneralLevelUp]);
}

#[cfg(feature = "game_client")]
#[test]
fn eva_announcement_owner_tests_frozen_context_preserves_defeated_local_player() {
    use crate::game_logic::{Player, Team};
    let mut world = GameLogic::new();
    world.add_player(Player::new(7, Team::China, "Local", true));
    let local = world.get_player_mut(7).unwrap();
    local.is_alive = false;
    local.power_consumed = 1;
    local.power_available = -1;
    world.set_current_frame(17);
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 7);
    let context = frame.eva_context;
    assert_eq!(context.frame, 17);
    let local = context
        .local_player
        .expect("CPP Eva.cpp270 checks local identity, not alive status");
    assert_eq!(local.index, 7);
    assert_eq!(local.side, "China");
    assert!(!local.sufficient_power);
    let absent = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 8);
    assert!(absent.eva_context.local_player.is_none());
}
