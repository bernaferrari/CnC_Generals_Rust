use super::*;
use crate::game_logic::radar_notifications::RadarKind;
use crate::presentation_frame::{PresentationEvent, PresentationFrame};

fn world_with_unit(position: Vec3) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("RadarOwnerProbe");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("RadarOwnerProbe", Team::USA, position)
        .expect("world-owned unit");
    (logic, id)
}

fn frozen_radar(logic: &mut GameLogic) -> Vec<(String, Vec3, u8)> {
    PresentationFrame::build_from_logic(logic, 0)
        .events
        .into_iter()
        .filter_map(|event| match event {
            PresentationEvent::RadarMessage {
                text,
                position,
                kind,
                ..
            } => Some((text, position, kind)),
            _ => None,
        })
        .collect()
}

#[test]
fn radar_owner_interleaved_freeze_and_ui_drain_stay_in_their_world() {
    let a_position = Vec3::new(10.0, 0.0, 20.0);
    let b_position = Vec3::new(90.0, 0.0, 80.0);
    let (mut a, a_id) = world_with_unit(a_position);
    a.queue_radar_message_at("A attack", a_position, RadarKind::Attack);
    let (mut b, b_id) = world_with_unit(b_position);
    assert_eq!(a_id, b_id, "identities may overlap across worlds");
    assert!(
        b.radar_notification_snapshot().is_empty(),
        "constructing B must not inherit A messages"
    );
    b.queue_radar_message_at("B ally", b_position, RadarKind::Ally);
    a.queue_radar_message_at("A generic", a_position, RadarKind::Generic);

    let expected_a = vec![
        ("A attack".into(), a_position, 1),
        ("A generic".into(), a_position, 0),
    ];
    assert_eq!(frozen_radar(&mut a), expected_a);
    assert_eq!(
        frozen_radar(&mut a),
        expected_a,
        "freeze must not drain or advance radar"
    );
    assert_eq!(frozen_radar(&mut b), vec![("B ally".into(), b_position, 2)]);
    let a_ui = a.update_ui_state(0);
    assert_eq!(a_ui.radar_messages, vec!["A attack", "A generic"]);
    assert_eq!(a_ui.radar_events[0].position, Some(a_position));
    assert!(a.radar_notification_snapshot().is_empty());
    assert_eq!(
        b.radar_notification_snapshot().len(),
        1,
        "draining A must leave B pending"
    );
    assert_eq!(b.update_ui_state(0).radar_messages, vec!["B ally"]);
    assert!(b.radar_notification_snapshot().is_empty());
}

#[test]
fn radar_owner_reset_clears_only_its_world_and_allows_reused_ids() {
    let (mut a, a_id) = world_with_unit(Vec3::X);
    let (mut b, b_id) = world_with_unit(Vec3::Z);
    assert_eq!(a_id, b_id);
    a.queue_radar_message_at("old A", Vec3::X, RadarKind::Attack);
    b.queue_radar_message_at("keep B", Vec3::Z, RadarKind::Attack);
    a.reset();
    assert!(
        a.radar_notification_snapshot().is_empty(),
        "reset must discard pending old-match messages"
    );
    assert_eq!(frozen_radar(&mut b), vec![("keep B".into(), Vec3::Z, 1)]);
    let new_id = a
        .create_object("RadarOwnerProbe", Team::USA, Vec3::Y)
        .expect("reused slot");
    assert_eq!(new_id, a_id);
    a.queue_radar_message_at("new A", Vec3::Y, RadarKind::Attack);
    assert_eq!(a.update_ui_state(0).radar_messages, vec!["new A"]);
    assert_eq!(b.update_ui_state(0).radar_messages, vec!["keep B"]);
}

#[test]
fn radar_owner_preserves_fifo_kind_dedup_and_timestamp_boundary() {
    let mut logic = GameLogic::new();
    logic.queue_radar_message_at("first", Vec3::X, RadarKind::Attack);
    logic.sim_time_seconds = 0.499;
    logic.queue_radar_message_at("suppressed", Vec3::Y, RadarKind::Attack);
    logic.queue_radar_message_at("independent kind", Vec3::Z, RadarKind::Ally);
    logic.sim_time_seconds = 0.5;
    logic.queue_radar_message_at("at boundary", Vec3::Y, RadarKind::Attack);
    let snapshot = logic.radar_notification_snapshot();
    assert_eq!(
        snapshot
            .iter()
            .map(|entry| entry.text.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "independent kind", "at boundary"]
    );
    assert_eq!(
        snapshot
            .iter()
            .map(|entry| entry.timestamp)
            .collect::<Vec<_>>(),
        vec![0.0, 0.499, 0.5]
    );
    assert_eq!(
        logic.update_ui_state(0).radar_messages,
        vec!["first", "independent kind", "at boundary"]
    );
    assert!(logic.radar_notification_snapshot().is_empty());
}

#[test]
fn radar_owner_snapshot_restore_into_staging_world_does_not_import_pending_ui_text() {
    use crate::save_load::snapshot::SnapshotBuilder;

    let (mut source, source_id) = world_with_unit(Vec3::X);
    let (mut neighbor, neighbor_id) = world_with_unit(Vec3::Z);
    assert_eq!(source_id, neighbor_id);
    source.queue_radar_message_at("source pending", Vec3::X, RadarKind::Attack);
    neighbor.queue_radar_message_at("neighbor pending", Vec3::Z, RadarKind::Attack);
    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("capture world");
    let (mut staging, staging_id) = world_with_unit(Vec3::ZERO);
    assert_eq!(staging_id, source_id);
    builder
        .restore_from_snapshot(&snapshot, &mut staging)
        .expect("restore staging world");

    // Pending UI delivery is not a WorldSnapshot field. The separately
    // serialized C++ Radar event ring remains covered by its own Xfer tests.
    assert!(staging.radar_notification_snapshot().is_empty());
    assert_eq!(
        frozen_radar(&mut source),
        vec![("source pending".into(), Vec3::X, 1)]
    );
    assert_eq!(
        frozen_radar(&mut neighbor),
        vec![("neighbor pending".into(), Vec3::Z, 1)]
    );
    staging.queue_radar_message_at("staging only", Vec3::Y, RadarKind::Ally);
    assert_eq!(
        staging.update_ui_state(0).radar_messages,
        vec!["staging only"]
    );
    assert_eq!(
        neighbor.update_ui_state(0).radar_messages,
        vec!["neighbor pending"]
    );
}
