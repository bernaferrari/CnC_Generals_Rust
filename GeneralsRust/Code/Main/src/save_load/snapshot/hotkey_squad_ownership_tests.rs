use super::*;

fn owned(ids: &[u32]) -> ControlGroupSave {
    ControlGroupSave {
        player_index: 0,
        groups: HashMap::from([(3, ids.to_vec())]),
    }
}

#[test]
fn owned_control_groups_interleaved_candidates_never_share_values() {
    let mut a = b"historical".to_vec();
    let mut b = a.clone();
    write_control_groups(&mut a, &owned(&[42, 43])).unwrap();
    write_control_groups(&mut b, &owned(&[42, 99])).unwrap();
    assert_eq!(read_control_groups(&a).unwrap().unwrap()[&0][&3], [42, 43]);
    assert_eq!(read_control_groups(&b).unwrap().unwrap()[&0][&3], [42, 99]);
    let pristine = b"historical".to_vec();
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
