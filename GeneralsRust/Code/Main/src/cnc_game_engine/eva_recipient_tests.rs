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

#[cfg(feature = "game_client")]
#[test]
fn named_requests_obey_ordered_enable_clears_and_invalid_names_do_not_play() {
    use game_client::eva::EvaMessage;
    let mut owner = HostEvaAnnouncements::default();
    let mut native = recipient();
    owner.request_named("BuildingLost");
    owner.set_enabled(false);
    owner.request_named("SuperweaponReady_Enemy_ScudStorm");
    owner.request_named("not_a_native_message");
    native.admit(&owner.take_delivery());
    assert!(!native.is_enabled());
    assert_eq!(native.speaker.pending_messages(), vec![EvaMessage::SuperweaponReadyEnemyScudStorm]);
    owner.set_enabled(true);
    owner.request(EvaEvent::UnitLost);
    native.admit(&owner.take_delivery());
    assert_eq!(native.speaker.pending_messages(), vec![EvaMessage::UnitLost]);
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

#[cfg(feature = "game_client")]
#[test]
fn authored_names_share_native_order_and_cooldown_with_typed_requests() {
    use game_client::eva::{Eva, EvaFrameContext, EvaLocalPlayerContext, EvaPlayback};
    #[derive(Default)]
    struct Speech(Vec<String>);
    impl EvaPlayback for Speech {
        fn is_playing(&mut self, _: u32) -> bool { false }
        fn play_speech(&mut self, event: &gamelogic::common::audio::AudioEventRts) -> u32 {
            self.0.push(event.event_name.clone());
            0
        }
    }
    let source = "EvaEvent SuperweaponReady_Ally_Nuke\nPriority = 1\nTimeBetweenChecksMS = 1000\nExpirationTimeMS = 5000\nSideSounds\nSide = America\nSounds = AllyNukeVoice\nEnd\nEnd\n\
EvaEvent GeneralLevelUp\nPriority = 1\nTimeBetweenChecksMS = 1000\nExpirationTimeMS = 5000\nSideSounds\nSide = America\nSounds = LevelVoice\nEnd\nEnd\n";
    let mut native = EvaRecipient::new(Eva::from_ordered_ini_sources([source]).unwrap());
    let mut world = crate::game_logic::GameLogic::new();
    world.add_player(crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "Local", true));
    world.add_player(crate::game_logic::Player::new(1, crate::game_logic::Team::China, "Enemy", false));
    let mut template = crate::game_logic::ThingTemplate::new("NamedStealthAgent");
    template.set_health(100.0);
    template.stealth_update = Some(crate::game_logic::object::StealthUpdateData::default());
    template.stealth_enemy_detection_eva = Some("superweaponready_ally_nuke".into());
    world.templates.insert("NamedStealthAgent".into(), template);
    let target = world.create_object("NamedStealthAgent", crate::game_logic::Team::China, glam::Vec3::ZERO).unwrap();
    world.eva_announcements.request(EvaEvent::GeneralLevelUp);
    world.try_eva_hero_detected(target);
    world.try_eva_hero_detected(target);
    native.admit(&world.eva_announcements.take_delivery());
    let mut speech = Speech::default();
    let context = |frame| EvaFrameContext {
        frame,
        local_player: Some(EvaLocalPlayerContext {
            side: "America".into(), index: 0, sufficient_power: true,
        }),
    };
    native.speaker.update_with_playback(&context(1), &mut speech, &mut |lo, _| lo);
    assert!(speech.0.is_empty());
    native.speaker.update_with_playback(&context(2), &mut speech, &mut |lo, _| lo);
    native.speaker.update_with_playback(&context(3), &mut speech, &mut |lo, _| lo);
    assert_eq!(speech.0, ["AllyNukeVoice", "LevelVoice"]);
    world.eva_announcements.request_named("SuperweaponReady_Ally_Nuke");
    native.admit(&world.eva_announcements.take_delivery());
    native.speaker.update_with_playback(&context(30), &mut speech, &mut |lo, _| lo);
    assert_eq!(speech.0.len(), 2);
    native.speaker.update_with_playback(&context(31), &mut speech, &mut |lo, _| lo);
    world.eva_announcements.request_named("SuperweaponReady_Ally_Nuke");
    native.admit(&world.eva_announcements.take_delivery());
    native.speaker.update_with_playback(&context(32), &mut speech, &mut |lo, _| lo);
    assert_eq!(speech.0, ["AllyNukeVoice", "LevelVoice", "AllyNukeVoice"]);
}
