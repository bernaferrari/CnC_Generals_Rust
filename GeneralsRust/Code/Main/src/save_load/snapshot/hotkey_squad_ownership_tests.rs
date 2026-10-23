use super::*;

fn owned(ids: &[u32]) -> ControlGroupSave {
    ControlGroupSave {
        player_index: 0,
        groups: HashMap::from([(3, ids.to_vec())]),
    }
}

#[test]
fn owned_control_groups_interleaved_candidates_never_share_values() {
    let mut a = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    let mut b = a.clone();
    write_control_groups(&mut a, &owned(&[42, 43])).unwrap();
    write_control_groups(&mut b, &owned(&[42, 99])).unwrap();
    assert_eq!(read_control_groups(&a).unwrap().unwrap()[&0][&3], [42, 43]);
    assert_eq!(read_control_groups(&b).unwrap().unwrap()[&0][&3], [42, 99]);
    let pristine = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    assert!(read_control_groups(&pristine).unwrap().is_none());
}

#[test]
fn owned_control_groups_empty_replaces_all_old_slots_and_keeps_later_extensions() {
    let mut bytes = Vec::new();
    write_control_groups(&mut bytes, &owned(&[42])).unwrap();
    bytes.extend_from_slice(b"LATER_EXTENSION");
    write_control_groups(
        &mut bytes,
        &ControlGroupSave {
            player_index: 0,
            groups: HashMap::new(),
        },
    )
    .unwrap();
    assert!(bytes.ends_with(b"LATER_EXTENSION"));
    assert!(read_control_groups(&bytes).unwrap().unwrap()[&0].is_empty());
}

#[test]
fn owned_control_groups_failed_candidate_does_not_change_valid_owner() {
    let mut valid = Vec::new();
    write_control_groups(&mut valid, &owned(&[42])).unwrap();
    let expected = valid.clone();
    let mut failed = valid.clone();
    failed.truncate(failed.len() - 1);
    assert!(read_control_groups(&failed).is_err());
    assert_eq!(valid, expected);
    assert_eq!(read_control_groups(&valid).unwrap().unwrap()[&0][&3], [42]);
    let mut invalid_owner = owned(&[7]);
    invalid_owner.groups.insert(10, vec![8]);
    assert!(write_control_groups(&mut valid, &invalid_owner).is_err());
    assert_eq!(valid, expected);
}

#[test]
fn control_groups_ignore_magic_inside_actual_named_history_and_keep_terminal_capsules() {
    let mut source = GameLogic::new();
    source.named_unit_history.insert("ValidHSQDUnitName".into());
    let mut snapshot = super::super::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    assert!(
        read_control_groups(&snapshot.lifecycle_tail)
            .unwrap()
            .is_none()
    );
    let prefix = super::super::player_team_chunks::lifecycle_prefix(&snapshot.lifecycle_tail)
        .unwrap()
        .len();
    let terminal = snapshot.lifecycle_tail[prefix..].to_vec();
    write_control_groups(&mut snapshot.lifecycle_tail, &owned(&[42, 9001, 42])).unwrap();
    assert!(snapshot.lifecycle_tail.ends_with(&terminal));
    assert_eq!(
        read_control_groups(&snapshot.lifecycle_tail)
            .unwrap()
            .unwrap()[&0][&3],
        [42, 9001, 42]
    );
    write_control_groups(&mut snapshot.lifecycle_tail, &owned(&[99])).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = GameLogic::new();
    super::super::SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.named_unit_history, source.named_unit_history);
    assert_eq!(
        read_control_groups(&decoded.lifecycle_tail)
            .unwrap()
            .unwrap()[&0][&3],
        [99]
    );
}

#[test]
fn control_groups_traverse_legacy_records_without_scanning_their_payloads() {
    let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    // An opaque module payload may contain even a completely valid HSQD frame.
    let mut decoy = Vec::new();
    write_control_groups(&mut decoy, &owned(&[123])).unwrap();
    bytes.extend_from_slice(b"MSRT");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(decoy.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&decoy);
    let prefix = bytes.clone();
    assert!(read_control_groups(&bytes).unwrap().is_none());
    write_control_groups(&mut bytes, &owned(&[42])).unwrap();
    assert!(bytes.starts_with(&prefix));
    assert_eq!(read_control_groups(&bytes).unwrap().unwrap()[&0][&3], [42]);
    write_control_groups(&mut bytes, &owned(&[9001])).unwrap();
    assert!(bytes.starts_with(&prefix));
    assert_eq!(
        read_control_groups(&bytes).unwrap().unwrap()[&0][&3],
        [9001]
    );
}

#[test]
fn control_groups_keep_supported_spcd_v1_and_v2_timer_names_opaque() {
    for version in [1u32, 2] {
        let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
        bytes.extend_from_slice(b"SPCD");
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes()); // player table
        bytes.extend_from_slice(&0u32.to_le_bytes()); // owner
        bytes.extend_from_slice(&1u32.to_le_bytes()); // timer count
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(b"HSQDName");
        bytes.extend_from_slice(&3.0f32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // object table
        if version == 2 {
            bytes.extend_from_slice(&0u32.to_le_bytes());
        } // pauses
        let prefix = bytes.clone();
        write_control_groups(&mut bytes, &owned(&[0, 999_999])).unwrap();
        assert!(bytes.starts_with(&prefix));
        assert_eq!(
            read_control_groups(&bytes).unwrap().unwrap()[&0][&3],
            [0, 999_999]
        );
    }
}
