//! Eva.cpp boolean admission, enum-order arbitration and frame gates.
use super::*;
struct SilentPlayback;
impl EvaPlayback for SilentPlayback {
    fn is_playing(&mut self, _: u32) -> bool {
        false
    }
    fn play_speech(&mut self, _: &AudioEventRts) -> u32 {
        0
    }
}
fn update(eva: &mut Eva, frame: u32) {
    eva.update_with_playback(&context(frame), &mut SilentPlayback, &mut |lo, _| lo);
}
fn context(frame: u32) -> EvaFrameContext {
    EvaFrameContext {
        frame,
        local_player: Some(EvaLocalPlayerContext {
            side: "China".into(),
            index: 7,
            sufficient_power: true,
        }),
    }
}
fn speaker() -> Eva {
    let mut eva = Eva::new();
    for name in ["UPGRADECOMPLETE", "GENERALLEVELUP"] {
        let info = eva.new_eva_check_info(name).expect("authored event");
        info.priority = 1;
        info.frames_between_checks = 30;
        info.frames_to_expire = 30;
        info.eva_side_sounds.push(EvaSideSounds {
            side: "China".into(),
            sound_names: vec![format!("China{name}")],
        });
    }
    eva
}
#[test]
fn eva_latch_order_enabled_clear_disabled_admission_and_early_frames() {
    let mut eva = speaker();
    eva.set_should_play(EvaMessage::GeneralLevelUp);
    eva.set_enabled(true);
    eva.set_should_play(EvaMessage::UpgradeComplete);
    update(&mut eva, 1);
    assert!(eva.checks.is_empty());
    assert!(eva.should_play[EvaMessage::UpgradeComplete.as_index()]);
    update(&mut eva, 2);
    assert_eq!(eva.checks.len(), 1);
    assert_eq!(eva.checks[0].eva_info, EvaMessage::UpgradeComplete);
    assert_eq!(eva.eva_speech.player_index, Some(7));
    assert_eq!(eva.eva_speech.get_event_name(), "ChinaUPGRADECOMPLETE");
    eva.set_should_play(EvaMessage::GeneralLevelUp);
    eva.set_enabled(true);
    assert!(!eva.should_play[EvaMessage::GeneralLevelUp.as_index()]);
    eva.set_should_play(EvaMessage::GeneralLevelUp);
    eva.set_enabled(true);
    assert!(!eva.should_play[EvaMessage::GeneralLevelUp.as_index()]);
    eva.set_enabled(false);
    eva.set_should_play(EvaMessage::GeneralLevelUp);
    update(&mut eva, 3);
    assert!(eva.should_play[EvaMessage::GeneralLevelUp.as_index()]);
    eva.set_enabled(true);
    assert!(!eva.should_play[EvaMessage::GeneralLevelUp.as_index()]);
}
#[test]
fn eva_explicit_context_coalesces_repeats_and_uses_enum_priority_order() {
    let mut eva = speaker();
    let first = if EvaMessage::UpgradeComplete.as_index() < EvaMessage::GeneralLevelUp.as_index() {
        EvaMessage::UpgradeComplete
    } else {
        EvaMessage::GeneralLevelUp
    };
    let second = if first == EvaMessage::UpgradeComplete {
        EvaMessage::GeneralLevelUp
    } else {
        EvaMessage::UpgradeComplete
    };
    eva.set_should_play(second);
    eva.set_should_play(first);
    eva.set_should_play(first);
    update(&mut eva, 2);
    assert_eq!(eva.checks.len(), 2);
    assert_eq!(eva.checks[0].eva_info, first);
    assert!(eva.checks[0].already_played);
    assert!(!eva.checks[1].already_played);
    assert_eq!(eva.played_count(), 1);
    let mut foreign = speaker();
    foreign.set_enabled(false);
    foreign.reset();
    drop(foreign);
    assert_eq!(eva.check_snapshot(&context(2)).1, 2);
}
