use generals_main::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
use generals_main::save_load::snapshot::SnapshotBuilder;
use glam::Vec3;

fn world() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.templates.insert(
        "GLATechnical".to_string(),
        ThingTemplate::new("GLATechnical"),
    );
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));
    let id = logic
        .create_object("GLATechnical", Team::GLA, Vec3::new(12.0, 0.0, 8.0))
        .expect("technical");
    (logic, id)
}

fn wsfl_record(tail: &[u8]) -> (usize, Vec<u8>) {
    let at = tail
        .windows(4)
        .enumerate()
        .filter(|(at, window)| {
            if *window != b"WSFL" || tail.len().saturating_sub(*at) < 12 {
                return false;
            }
            let version = u32::from_le_bytes(tail[*at + 4..*at + 8].try_into().unwrap());
            let length = u32::from_le_bytes(tail[*at + 8..*at + 12].try_into().unwrap()) as usize;
            version == 1
                && at
                    .checked_add(12)
                    .and_then(|n| n.checked_add(length))
                    .is_some_and(|end| end <= tail.len())
        })
        .map(|(at, _)| at)
        .next()
        .expect("snapshot writes WSFL");
    let length = u32::from_le_bytes(tail[at + 8..at + 12].try_into().unwrap()) as usize;
    let end = at + 12 + length;
    assert!(
        end <= tail.len(),
        "WSFL frame is bounded by the lifecycle tail"
    );
    (at, tail[at..end].to_vec())
}

fn receiving_world() -> (GameLogic, ObjectId) {
    world()
}

#[test]
fn weapon_set_flags_round_trip_in_a_real_snapshot() {
    let (mut source, source_id) = world();
    let technical = source.host_object_mut(source_id).unwrap();
    technical.apply_salvage_weapon_upgrade();
    // OXOB is a later framed record. Its legitimate payload can contain the
    // WSFL tag as data and must not be mistaken for another WSFL header.
    technical.safe_occlusion_frame = u32::from_le_bytes(*b"WSFL");
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
    let (wsfl_at, wsfl) = wsfl_record(&snapshot.lifecycle_tail);
    assert!(
        snapshot.lifecycle_tail[wsfl_at + wsfl.len()..]
            .windows(4)
            .any(|bytes| bytes == b"WSFL")
    );

    let (mut restored, _) = receiving_world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore snapshot");
    let loaded = restored.host_object(source_id).expect("restored technical");
    assert_eq!(loaded.weapon_crate_upgrade, 1);
    assert_eq!(loaded.safe_occlusion_frame, u32::from_le_bytes(*b"WSFL"));
    assert!(
        loaded
            .applied_upgrades
            .contains("WEAPONSET_CRATEUPGRADE_ONE")
    );
}

#[test]
fn malformed_or_duplicate_wsfl_is_rejected_before_receiving_world_mutation() {
    for malformed_kind in ["duplicate", "truncated"] {
        let (mut source, source_id) = world();
        source
            .host_object_mut(source_id)
            .unwrap()
            .apply_salvage_weapon_upgrade();
        let builder = SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let (_, record) = wsfl_record(&snapshot.lifecycle_tail);
        if malformed_kind == "duplicate" {
            snapshot.lifecycle_tail.extend_from_slice(&record);
        } else {
            snapshot.lifecycle_tail.extend_from_slice(b"WSFL");
            snapshot
                .lifecycle_tail
                .extend_from_slice(&1u32.to_le_bytes());
        }

        let (mut receiving, receiving_id) = receiving_world();
        let object_count = receiving.host_objects().registered_keys().count();
        let before_upgrade = receiving
            .host_object(receiving_id)
            .unwrap()
            .weapon_crate_upgrade;
        assert!(
            builder
                .restore_from_snapshot(&snapshot, &mut receiving)
                .is_err()
        );
        assert_eq!(
            receiving.host_objects().registered_keys().count(),
            object_count,
            "invalid WSFL framing must be rejected before replacing receiving objects"
        );
        assert_eq!(
            receiving
                .host_object(receiving_id)
                .unwrap()
                .weapon_crate_upgrade,
            before_upgrade
        );
    }
}
