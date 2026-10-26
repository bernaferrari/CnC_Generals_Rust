//! Actual World capture/Xfer/restore, with no device or process mixer publication.
use super::*;
use game_engine::common::audio::game_audio::AudioAffect;
fn wire(mut snapshot: WorldSnapshot) -> WorldSnapshot {
    use crate::save_load::{XferData, XferLoad, XferSave};
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut save = XferSave::new(&mut bytes);
    snapshot.xfer(&mut save).unwrap();
    drop(save);
    let mut restored = WorldSnapshot::default();
    restored
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    restored
}
#[test]
fn script_audio_policy_roundtrip_continues_new_admission_and_peer_reset_is_independent() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    a.queued_audio_events
        .set_volume(0.35, AudioAffect::SoundEffects);
    a.queued_audio_events.set_named_volume("LateCue", 0.6);
    b.queued_audio_events.set_volume(0.9, AudioAffect::Music);
    let builder = SnapshotBuilder::new();
    let snapshot = wire(builder.create_world_snapshot(&a).unwrap());
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.queued_audio_events.policy(),
        a.queued_audio_events.policy()
    );
    b.reset();
    assert_eq!(a.queued_audio_events.policy().volumes.sound, 0.35);
    restored.queue_audio_event(crate::game_logic::AudioEventRequest::new("LateCue"));
    let captured = restored.queued_audio_events[0]
        .script_audio_policy
        .as_ref()
        .unwrap();
    assert_eq!(captured.override_for("LateCue"), Some(0.6));
    assert_eq!(captured.volumes.sound_3d, 0.35);
    restored
        .queued_audio_events
        .set_named_volume("LateCue", -1.);
    assert_eq!(
        a.queued_audio_events.policy().override_for("LateCue"),
        Some(0.6)
    );
}
#[test]
fn malformed_audio_policy_rejects_before_receiver_state_changes() {
    let source = GameLogic::new();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    snapshot.audio_policy.policy.volumes.sound = f32::NAN;
    let mut receiving = GameLogic::new();
    receiving.queued_audio_events.set_named_volume("Keep", 0.25);
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .is_err()
    );
    assert_eq!(
        receiving.queued_audio_events.policy().override_for("Keep"),
        Some(0.25)
    );
    snapshot.audio_policy.policy.volumes.sound = 1.;
    snapshot.audio_policy.version = 2;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .is_err()
    );
    assert_eq!(
        receiving.queued_audio_events.policy().override_for("Keep"),
        Some(0.25)
    );
}
