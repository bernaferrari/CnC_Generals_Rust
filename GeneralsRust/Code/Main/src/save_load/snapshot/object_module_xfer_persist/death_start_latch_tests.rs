//! Encoding controls for the Rust functional onDie latch; not C++ wire fields.
use super::*;
use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;
use glam::Vec3;

fn fixture() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut source = GameLogic::new();
    source.add_player(Player::new(0, Team::USA, "Latch owner", true));
    let mut oil = ThingTemplate::new("TechOilDerrick");
    oil.set_health(500.0).add_kind_of(KindOf::Structure);
    source.templates.insert("TechOilDerrick".into(), oil);
    source
        .templates
        .insert("LatchInactive".into(), ThingTemplate::new("LatchInactive"));
    let kept = source
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let inactive = source
        .create_object("LatchInactive", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let phase_only = source
        .create_object("LatchInactive", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    // Exercise the existing actual KeepObject death-start dispatch. The other
    // two rows are encoding controls: phase/HP must never infer the latch.
    source.mark_object_for_destruction(kept, None);
    let kept_object = source.host_object(kept).unwrap();
    assert!(kept_object.status.on_die_started);
    assert!(kept_object.status.keep_as_rubble && !kept_object.status.destroyed);
    assert!(kept_object.slow_death.is_none());
    let object = source.host_object_mut(phase_only).unwrap();
    let mut death = HostSlowDeathData::default();
    death.phase = crate::game_logic::host_slow_death::HostSlowDeathPhase::WaitingToSink;
    object.slow_death = Some(death);
    object.health.current = 0.0;
    assert!(!object.status.on_die_started);
    assert!(!source.host_object(inactive).unwrap().status.on_die_started);
    (source, kept, inactive, phase_only)
}

#[test]
fn oxfr_death_start_exact_bool_survives_snapshot_builder_for_keepobject_and_inactive() {
    let (source, kept, inactive, phase_only) = fixture();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(
        restored.host_object(kept).unwrap().status.on_die_started,
        "restored actual KeepObject must retain the functional callback latch"
    );
    assert!(
        !restored
            .host_object(inactive)
            .unwrap()
            .status
            .on_die_started
    );
    assert!(
        !restored
            .host_object(phase_only)
            .unwrap()
            .status
            .on_die_started,
        "death phase and dead HP cannot invent a callback latch"
    );
    assert_eq!(
        restored
            .host_object(phase_only)
            .unwrap()
            .slow_death
            .as_ref()
            .unwrap()
            .phase,
        crate::game_logic::host_slow_death::HostSlowDeathPhase::WaitingToSink
    );
    assert!(source.host_object(kept).unwrap().status.on_die_started);
}

#[test]
fn oxfr_death_start_records_follow_existing_module_vectors_in_sorted_order_and_keep_sentinel() {
    let (source, kept, inactive, phase_only) = fixture();
    let payload = capture(&source);
    // Four original vectors retain their exact relative wire order and bytes.
    let prefix = bincode_legacy::serialize(&(
        &payload.stun,
        &payload.battle_bus,
        &payload.slow_death,
        &payload.radar,
    ))
    .unwrap();
    let mut bytes = b"earlier-lifecycle-domain".to_vec();
    let prefix_len = bytes.len();
    append_to_lifecycle_tail(&mut bytes, &source);
    let following = 0x91A2_B3C4u32;
    bytes.extend_from_slice(&following.to_le_bytes());
    assert_eq!(&bytes[..prefix_len], b"earlier-lifecycle-domain");
    let mut suffix = find_oxfr_suffix(&bytes).unwrap();
    let version = take_u32(&mut suffix).unwrap();
    assert_eq!(
        version, 2,
        "Rust lifecycle domain version must describe the appended record"
    );
    let payload_len = take_u32(&mut suffix).unwrap() as usize;
    let encoded = &suffix[..payload_len];
    assert!(
        encoded.starts_with(&prefix),
        "original four module vectors changed"
    );
    let records: Vec<(u32, bool)> = bincode_legacy::deserialize(&encoded[prefix.len()..]).unwrap();
    assert_eq!(
        records,
        vec![(kept.0, true), (inactive.0, false), (phase_only.0, false)]
    );
    assert_eq!(&suffix[payload_len..], &following.to_le_bytes());
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    // Set stale values to check the encoded false rows are applied explicitly.
    restored
        .host_object_mut(inactive)
        .unwrap()
        .status
        .on_die_started = true;
    restored
        .host_object_mut(phase_only)
        .unwrap()
        .status
        .on_die_started = true;
    apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
    assert!(restored.host_object(kept).unwrap().status.on_die_started);
    assert!(
        !restored
            .host_object(inactive)
            .unwrap()
            .status
            .on_die_started
    );
    assert!(
        !restored
            .host_object(phase_only)
            .unwrap()
            .status
            .on_die_started
    );
}

#[test]
fn oxfr_v1_rejection_precedes_domain_state_application() {
    let (source, kept, _, _) = fixture();
    let payload = capture(&source);
    let legacy = bincode_legacy::serialize(&(
        &payload.stun,
        &payload.battle_bus,
        &payload.slow_death,
        &payload.radar,
    ))
    .unwrap();
    let mut bytes = OXFR_MAGIC.to_vec();
    append_u32(&mut bytes, 1);
    append_u32(&mut bytes, legacy.len() as u32);
    bytes.extend_from_slice(&legacy);
    let mut target = GameLogic::new();
    target.templates = source.templates.clone();
    let id = target
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .unwrap();
    assert_eq!(id, kept);
    let error = apply_from_lifecycle_tail(&bytes, &mut target).unwrap_err();
    assert!(matches!(error, SaveLoadError::Corrupted(ref message)
        if message == "unknown OXFR suffix version 1"));
    assert!(!target.host_object(id).unwrap().status.on_die_started);
    assert!(target.host_object(id).unwrap().slow_death.is_none());
}
