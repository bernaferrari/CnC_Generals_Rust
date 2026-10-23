use super::tests::{owned_a10_world, rewrite_capsule};
use super::*;
use crate::save_load::snapshot::SnapshotBuilder;

#[test]
fn encoded_world_snapshot_restores_effective_lighting_but_keeps_receiver_baseline() {
    use game_engine::common::ini::ini_game_data::TimeOfDay;
    let mut source = owned_a10_world();
    let mut source_baseline = crate::game_logic::lighting_rules::LightingContentRules::default();
    source_baseline.time_of_day = TimeOfDay::Morning;
    source_baseline.num_global_lights = 2;
    source.admit_lighting_rules_base(source_baseline);
    source.set_receiving_time_of_day(TimeOfDay::Night);
    let source_id = source
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source_id,
            glam::Vec3::new(400.0, 0.0, 500.0),
        )
        .unwrap();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiver = owned_a10_world();
    let mut receiver_baseline = crate::game_logic::lighting_rules::LightingContentRules::default();
    receiver_baseline.time_of_day = TimeOfDay::Afternoon;
    receiver_baseline.num_global_lights = 3;
    receiver.admit_lighting_rules_base(receiver_baseline);
    builder
        .restore_from_snapshot(&decoded, &mut receiver)
        .unwrap();
    assert_eq!(receiver.lighting_rules().time_of_day, TimeOfDay::Night);
    assert_eq!(receiver.lighting_rules().num_global_lights, 2);
    receiver.reset();
    assert_eq!(receiver.lighting_rules().time_of_day, TimeOfDay::Afternoon);
    assert_eq!(receiver.lighting_rules().num_global_lights, 3);
}

#[test]
fn malformed_lighting_capsule_rejects_before_objects_audio_or_lighting_change() {
    use game_engine::common::ini::ini_game_data::TimeOfDay;
    let mut source = owned_a10_world();
    let source_id = source
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source_id,
            glam::Vec3::new(400.0, 0.0, 500.0),
        )
        .unwrap();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    rewrite_capsule(&mut snapshot.lifecycle_tail, |payload| {
        payload.lighting.version = u32::MAX;
    });

    let mut receiver = owned_a10_world();
    let receiver_source = receiver
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    receiver.queue_audio_event(crate::game_logic::AudioEventRequest::new(
        "KeepReceiverAudio",
    ));
    receiver.set_receiving_time_of_day(TimeOfDay::Evening);
    let before_ids: Vec<_> = receiver.objects.registered_keys().copied().collect();
    let before_hp = receiver.objects[&receiver_source].health.current;
    let before_audio: Vec<_> = receiver
        .queued_audio_events
        .iter()
        .map(|event| event.event_type.clone())
        .collect();
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(
        receiver
            .objects
            .registered_keys()
            .copied()
            .collect::<Vec<_>>(),
        before_ids
    );
    assert_eq!(receiver.objects[&receiver_source].health.current, before_hp);
    assert_eq!(
        receiver
            .queued_audio_events
            .iter()
            .map(|event| event.event_type.clone())
            .collect::<Vec<_>>(),
        before_audio
    );
    assert_eq!(receiver.lighting_rules().time_of_day, TimeOfDay::Evening);
}
