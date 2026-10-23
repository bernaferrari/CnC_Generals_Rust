//! CPAD status is independent of BuildAssistant sale queues and peer worlds.
use super::*;
use crate::game_logic::{Player, Team, ThingTemplate};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "CPAD", true));
    world
        .templates
        .insert("StatusUnit".into(), ThingTemplate::new("StatusUnit"));
    let first = world
        .create_object_for_player("StatusUnit", 0, Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object_for_player("StatusUnit", 0, Vec3::X)
        .unwrap();
    (world, first, second)
}
fn state(world: &GameLogic) -> Vec<(u32, bool, bool, bool)> {
    let mut values: Vec<_> = world
        .host_objects()
        .registered_values()
        .map(|o| {
            (
                o.id.0,
                o.construction_power_admitted,
                o.status.sold,
                o.status.reconstructing,
            )
        })
        .collect();
    values.sort_unstable_by_key(|row| row.0);
    values
}
fn capsule(version: u32, rows: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = (rows.len() as u32).to_le_bytes().to_vec();
    for (id, flags) in rows {
        bytes.extend_from_slice(&id.to_le_bytes());
        if version != 1 {
            bytes.extend_from_slice(&flags.to_le_bytes());
        }
    }
    let length = bytes.len() as u32;
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(MAGIC);
    bytes
}
#[test]
fn flags_restore_without_sale_queue_or_peer_mutation() {
    let (mut source, id, control) = world();
    let (mut receiving, peer_id, peer_control) = world();
    let (peer, _, _) = world();
    assert_eq!((id, control), (peer_id, peer_control));
    let peer_before = state(&peer);
    {
        let object = source.host_object_mut(id).unwrap();
        object.construction_power_admitted = true;
        object.status.sold = true;
        object.status.reconstructing = true;
    }
    // No sale command or sell_list admission: only canonical Object flags.
    receiving.host_object_mut(control).unwrap().status.sold = true;
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    assert_eq!(bytes, capsule(2, &[(id.0, 7)]));
    apply_from_lifecycle_tail(&bytes, &mut receiving).unwrap();
    assert_eq!(state(&receiving), state(&source));
    assert_eq!(state(&peer), peer_before);
    assert_eq!(receiving.script_object_count_changed_frame, 0);
}
#[test]
fn v1_keeps_exact_id_only_status_interpretation() {
    let (mut receiving, id, control) = world();
    receiving.host_object_mut(id).unwrap().status.sold = true;
    receiving
        .host_object_mut(control)
        .unwrap()
        .status
        .reconstructing = true;
    receiving
        .host_object_mut(control)
        .unwrap()
        .construction_power_admitted = true;
    apply_from_lifecycle_tail(&capsule(1, &[(id.0, 0)]), &mut receiving).unwrap();
    assert!(
        receiving
            .host_object(id)
            .unwrap()
            .construction_power_admitted
    );
    assert!(receiving.host_object(id).unwrap().status.sold);
    assert!(
        !receiving
            .host_object(control)
            .unwrap()
            .construction_power_admitted
    );
    assert!(
        receiving
            .host_object(control)
            .unwrap()
            .status
            .reconstructing
    );
    apply_from_lifecycle_tail(b"historical", &mut receiving).unwrap();
    assert!(
        !receiving
            .host_object(id)
            .unwrap()
            .construction_power_admitted
    );
    assert!(receiving.host_object(id).unwrap().status.sold);
    assert!(
        receiving
            .host_object(control)
            .unwrap()
            .status
            .reconstructing
    );
}
#[test]
fn malformed_capsules_leave_every_receiving_flag_unchanged() {
    let (mut receiving, id, control) = world();
    receiving
        .host_object_mut(control)
        .unwrap()
        .construction_power_admitted = true;
    receiving.host_object_mut(control).unwrap().status.sold = true;
    receiving.host_object_mut(id).unwrap().status.reconstructing = true;
    let before = state(&receiving);
    let mut bad = vec![
        capsule(3, &[(id.0, 7)]),
        capsule(2, &[(id.0, 1), (id.0, 2)]),
        capsule(1, &[(id.0, 0), (id.0, 0)]),
        capsule(2, &[(u32::MAX, 7)]),
        capsule(1, &[(u32::MAX, 0)]),
        capsule(2, &[(id.0, 8)]),
    ];
    let mut wrong_count = capsule(2, &[(id.0, 7)]);
    wrong_count[..4].copy_from_slice(&2u32.to_le_bytes());
    bad.push(wrong_count);
    let mut wrong_length = capsule(2, &[(id.0, 7)]);
    let offset = wrong_length.len() - 8;
    wrong_length[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    bad.push(wrong_length);
    bad.push(b"CPAD".to_vec());
    for bytes in bad {
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        assert_eq!(state(&receiving), before);
    }
}

#[test]
fn direct_world_xfer_restores_owned_status_without_sale_queue_or_peer_callbacks() {
    use crate::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
    use std::io::Cursor;

    let (mut source, id, control) = world();
    let (mut receiving, receiving_id, receiving_control) = world();
    let (peer, peer_id, peer_control) = world();
    assert_eq!((id, control), (receiving_id, receiving_control));
    assert_eq!((id, control), (peer_id, peer_control));
    source.set_current_frame(73);
    {
        let object = source.host_object_mut(id).unwrap();
        object.construction_power_admitted = true;
        object.status.sold = true;
        object.status.reconstructing = true;
    }
    // Deliberately no sale command: BuildAssistant's queue is not authority
    // for Object's independently saved SOLD/reconstructing continuation.
    {
        let object = receiving.host_object_mut(control).unwrap();
        object.construction_power_admitted = true;
        object.status.sold = true;
        object.status.reconstructing = true;
    }
    let source_before = state(&source);
    let peer_before = state(&peer);
    let peer_frame = peer.get_current_frame();
    let peer_callback_frame = peer.script_object_count_changed_frame;
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut encoded = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut encoded)).unwrap();
    let mut decoded = WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(Cursor::new(encoded.into_inner())))
        .unwrap();
    assert_eq!(decoded.lifecycle_tail, snapshot.lifecycle_tail);
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(receiving.get_current_frame(), 73);
    assert_eq!(state(&receiving), source_before);
    assert_eq!(state(&source), source_before);
    assert_eq!(state(&peer), peer_before);
    assert_eq!(peer.get_current_frame(), peer_frame);
    assert_eq!(peer.script_object_count_changed_frame, peer_callback_frame);
    assert_eq!(receiving.script_object_count_changed_frame, 0);
    assert_eq!(source.script_object_count_changed_frame, 0);
}
