use super::*;
use crate::game_logic::{Player, Team, ThingTemplate};
use crate::save_load::SnapshotBuilder;
use glam::Vec3;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "OrderOwner", true));
    let mut template = ThingTemplate::new("SavedListUnit");
    template.set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    for id in [700, 9, 300] {
        world.set_next_object_id_for_restore(ObjectId(id));
        world
            .create_object_for_player("SavedListUnit", 0, Vec3::ZERO)
            .unwrap();
    }
    world
}

fn capsule(ids: &[u32]) -> Vec<u8> {
    let mut bytes = TAG.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(ids.len() as u32 * 4).to_le_bytes());
    for id in ids {
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes
}

#[test]
fn snapshot_admits_records_in_saved_list_order_like_original_xfer() {
    let source = world();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    assert_eq!(
        source.objects.ordered_ids(),
        &[ObjectId(300), ObjectId(9), ObjectId(700)]
    );
    let mut restored = GameLogic::new();
    restored.add_player(Player::new(0, Team::USA, "OrderOwner", true));
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    // C++ writes head→next, then newObject/registerObject prepends every
    // loaded record. No later loadPostProcess reverses that original list.
    assert_eq!(
        restored.objects.ordered_ids(),
        &[ObjectId(700), ObjectId(9), ObjectId(300)]
    );
    restored.set_next_object_id_for_restore(ObjectId(80));
    restored
        .create_object_for_player("SavedListUnit", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(restored.objects.ordered_ids()[0], ObjectId(80));
    assert_eq!(source.objects.ordered_ids()[0], ObjectId(300));
}

#[test]
fn restored_team_roster_follows_object_admission_even_after_a_member_rejoined() {
    let source = world();
    let team_id = source.host_default_team_id(0).expect("source Team");
    let source_team = source
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    {
        let mut team = source_team.write().unwrap();
        // Leaving and rejoining changes Team order independently of the
        // global Object list. Reversing the saved Team rows would also fail.
        team.remove_member(9);
        team.add_member(9);
        assert_eq!(team.get_members(), &[9, 300, 700]);
    }
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = GameLogic::new();
    restored.add_player(Player::new(0, Team::USA, "OrderOwner", true));
    restored.templates = source.templates.clone();
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

    let restored_team = restored
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    // Object.cpp:853 prepends at setOrRestoreTeam. Team.cpp:2679–2724
    // only checks the incoming IDs and performs no later reorder.
    assert_eq!(restored_team.read().unwrap().get_members(), &[700, 9, 300]);
    assert_eq!(source_team.read().unwrap().get_members(), &[9, 300, 700]);
}

#[test]
fn order_codec_validates_exact_coverage_and_never_scans_opaque_bytes() {
    let world = world();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    for ids in [
        vec![700, 9],
        vec![700, 9, 9],
        vec![700, 9, 0],
        vec![700, 9, 444],
    ] {
        let mut bytes = vec![0; 12]; // empty lifecycle envelopes + two link lists
        bytes.extend(capsule(&ids));
        let decoded = decode(&bytes).unwrap();
        assert!(validate(decoded.saved_order.as_deref(), &snapshot.objects).is_err());
    }
    let mut bytes = vec![0; 12];
    bytes.extend(capsule(&[300, 9, 700]));
    let decoded = decode(&bytes).unwrap();
    validate(decoded.saved_order.as_deref(), &snapshot.objects).unwrap();
    assert_eq!(decoded.legacy_tail.as_ref(), &[0; 12]);
    let mut legacy = vec![1, 0, 0, 0, 4, 0, 0, 0];
    legacy.extend(TAG); // opaque four-byte lifecycle envelope
    assert!(decode(&legacy).unwrap().saved_order.is_none());
    assert!(decode(&[0; 12]).unwrap().saved_order.is_none());
}

#[test]
fn truncated_duplicate_and_unknown_order_capsules_are_rejected() {
    let complete = capsule(&[9]);
    for end in 1..complete.len() {
        let mut bytes = vec![0; 12];
        bytes.extend(&complete[..end]);
        assert!(decode(&bytes).is_err(), "accepted truncated prefix {end}");
    }
    let mut bytes = vec![0; 12];
    bytes.extend(&complete);
    bytes.extend(&complete);
    assert!(decode(&bytes).is_err());
    let mut bytes = vec![0; 12];
    let mut unknown = complete;
    unknown[4..8].copy_from_slice(&2u32.to_le_bytes());
    bytes.extend(unknown);
    assert!(decode(&bytes).is_err());
}

#[test]
fn invalid_order_is_rejected_before_mutating_receiving_world() {
    let source = world();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let start = snapshot
        .lifecycle_tail
        .windows(4)
        .position(|word| word == TAG)
        .unwrap();
    // Duplicate the first record without changing framing or length.
    let first = snapshot.lifecycle_tail[start + 12..start + 16].to_vec();
    snapshot.lifecycle_tail[start + 16..start + 20].copy_from_slice(&first);
    let mut receiving = world();
    receiving.set_current_frame(713);
    let before = receiving.objects.ordered_ids().to_vec();
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .is_err()
    );
    assert_eq!(receiving.frame, 713);
    assert_eq!(receiving.objects.ordered_ids(), before);
}
