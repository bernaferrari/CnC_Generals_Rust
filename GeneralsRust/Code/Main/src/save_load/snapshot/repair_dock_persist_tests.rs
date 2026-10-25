use super::*;
use crate::game_logic::{Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;

fn owner(frames: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("RepairCacheOwner");
    template.repair_dock_frames_for_full_heal = Some(frames);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("RepairCacheOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}
fn capsule_offset(bytes: &[u8]) -> usize {
    let mut offset = super::super::combat_projectile_persist::prefix_len(bytes).unwrap();
    loop {
        let tag: &[u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
        if tag == TAG {
            break;
        }
        assert!([b"CPRJ", b"PCOI", b"GMSH", b"BDKD", b"NLFC", b"ODIS"].contains(&tag));
        let len = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
        offset += 12 + len;
    }
    offset
}
fn replace_records(snapshot: &mut super::super::WorldSnapshot, records: &[Record]) {
    let bytes = &snapshot.lifecycle_tail;
    let offset = capsule_offset(bytes);
    let old_len = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
    let payload = bincode_legacy::serialize(records).unwrap();
    let mut replaced = bytes[..offset].to_vec();
    replaced.extend_from_slice(TAG);
    replaced.extend_from_slice(&VERSION.to_le_bytes());
    replaced.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    replaced.extend_from_slice(&payload);
    replaced.extend_from_slice(&bytes[offset + 12 + old_len..]);
    snapshot.lifecycle_tail = replaced;
}

#[test]
fn repair_cache_roundtrip_preserves_zero_rate_fractional_duration_and_peer() {
    let (mut source, id) = owner(2.5);
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .restore_repair_dock_state(2.5, Some(ObjectId(900)), 0.0);
    let (peer, peer_id) = owner(7.0);
    assert_eq!(id, peer_id);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let object = &restored.objects[&id];
    assert_eq!(
        object.thing().template.repair_dock_frames_for_full_heal,
        Some(2.5)
    );
    assert_eq!(object.repair_dock_last_id, Some(ObjectId(900)));
    assert_eq!(
        object.repair_dock_health_per_frame.to_bits(),
        0.0f32.to_bits()
    );
    assert_eq!(
        peer.objects[&id]
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(7.0)
    );
    assert_eq!(peer.objects[&id].repair_dock_last_id, None);
}

#[test]
fn repair_cache_raw_float_values_and_historical_ids_survive() {
    for (frames, rate) in [
        (0.0, f32::INFINITY),
        (0.0, f32::from_bits(0x7fc01234)),
        (-2.0, -4.0),
    ] {
        let (mut world, id) = owner(frames);
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .restore_repair_dock_state(frames, Some(ObjectId(u32::MAX)), rate);
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        let object = &restored.objects[&id];
        assert_eq!(object.repair_dock_last_id, Some(ObjectId(u32::MAX)));
        assert_eq!(
            object.repair_dock_health_per_frame.to_bits(),
            rate.to_bits()
        );
        assert_eq!(
            object
                .thing()
                .template
                .repair_dock_frames_for_full_heal
                .unwrap()
                .to_bits(),
            frames.to_bits()
        );
    }
}

#[test]
fn repair_cache_rejects_duplicates_unknown_owners_nonfinite_config_before_mutation() {
    let (source, id) = owner(3.0);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let record = Record {
        object_id: id.0,
        frames: 3.0,
        last_repair: 999,
        health_per_frame: 0.5,
    };
    let mut invalid_records = vec![vec![record.clone(), record.clone()], vec![]];
    let mut unknown = record.clone();
    unknown.object_id = 999;
    invalid_records.push(vec![unknown]);
    let mut invalid = record;
    invalid.frames = f32::NAN;
    invalid_records.push(vec![invalid]);
    for records in invalid_records {
        let mut corrupt = super::super::decode_bincode_world_snapshot(
            &bincode_legacy::serialize(&snapshot).unwrap(),
        )
        .unwrap();
        replace_records(&mut corrupt, &records);
        let (mut receiver, same_id) = owner(3.0);
        receiver
            .objects
            .get_mut(&same_id)
            .unwrap()
            .restore_repair_dock_state(3.0, Some(ObjectId(42)), 9.0);
        receiver.frame = 77;
        assert!(
            builder
                .restore_from_snapshot(&corrupt, &mut receiver)
                .is_err()
        );
        assert_eq!(receiver.frame, 77);
        assert_eq!(
            receiver.objects[&same_id].repair_dock_last_id,
            Some(ObjectId(42))
        );
        assert_eq!(receiver.objects[&same_id].repair_dock_health_per_frame, 9.0);
    }
}

#[test]
fn repair_cache_receiving_definition_mismatch_is_transactional() {
    let (source, _) = owner(3.0);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut receiver, id) = owner(4.0);
    receiver.frame = 91;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(receiver.frame, 91);
    assert_eq!(
        receiver.objects[&id]
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(4.0)
    );
}

#[test]
fn repair_cache_absent_legacy_capsule_resets_previous_session_only() {
    let (mut receiving, id) = owner(3.0);
    receiving
        .objects
        .get_mut(&id)
        .unwrap()
        .restore_repair_dock_state(3.0, Some(ObjectId(42)), 9.0);
    let empty = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    let decoded = decode(&empty).unwrap();
    assert!(decoded.records.is_none());
    apply(decoded.records.as_deref(), &mut receiving);
    assert_eq!(receiving.objects[&id].repair_dock_last_id, None);
    assert_eq!(receiving.objects[&id].repair_dock_health_per_frame, 0.0);
    assert_eq!(
        receiving.objects[&id]
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(3.0)
    );
}

#[test]
fn repair_cache_structural_decoder_rejects_truncation_version_and_duplicates() {
    let (world, _) = owner(1.0);
    let prefix = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    let mut bytes = prefix.clone();
    append(&mut bytes, &world).unwrap();
    for end in prefix.len() + 1..bytes.len() {
        assert!(
            decode(&bytes[..end]).is_err(),
            "accepted truncated capsule at {end}"
        );
    }
    let mut version = bytes.clone();
    version[prefix.len() + 4..prefix.len() + 8].copy_from_slice(&2u32.to_le_bytes());
    assert!(decode(&version).is_err());
    let duplicate = bytes[prefix.len()..].to_vec();
    bytes.extend_from_slice(&duplicate);
    assert!(decode(&bytes).is_err());
}

#[test]
fn repair_cache_save_load_continues_real_docked_action_without_recomputing_rate() {
    use crate::game_logic::{AIState, Player};
    let mut parser = crate::assets::IniParser::new();
    let source_text = "Object SavedRepairDock\n KindOf = VEHICLE REPAIR_PAD\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 20\n End\n Behavior = RepairDockUpdate RepairTag\n TimeForFullHeal = 100\n End\nEnd\n";
    parser
        .parse_ini_content(source_text, "saved-repair-dock.ini")
        .unwrap();
    let template = GameLogic::build_template_from_object_definition(
        "SavedRepairDock",
        parser.get_definition("SavedRepairDock").unwrap(),
        None,
    );
    assert_eq!(template.repair_dock_frames_for_full_heal, Some(3.0));
    let mut source = GameLogic::new();
    source.add_player(Player::new(1, Team::USA, "SavedRepairOwner", true));
    source.templates.insert(template.name.clone(), template);
    source
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "saved-repair-dock.ini".into(),
                source_text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    let dock = source
        .create_object_for_player("SavedRepairDock", 1, glam::Vec3::ZERO)
        .unwrap();
    let master = source
        .create_object_for_player("SavedRepairDock", 1, glam::Vec3::ZERO)
        .unwrap();
    {
        let object = source.objects.get_mut(&master).unwrap();
        object.target = Some(dock);
        object.set_ai_state(
            AIState::SeekingRepair,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    source.objects.get_mut(&dock).unwrap().dock_active_docker = Some(master);
    source.frame = 10;
    source.update_support_states_for_test(&[master], 1.0 / 30.0);
    assert!(source.objects[&master].health.current > 20.0);
    assert!(source.objects[&master].health.current < 100.0);
    let rate = source.objects[&dock].repair_dock_health_per_frame;
    assert_eq!(source.objects[&dock].repair_dock_last_id, Some(master));
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = GameLogic::new();
    restored.add_player(Player::new(1, Team::USA, "SavedRepairOwner", true));
    // GameLogic::xfer resolves Object TOC names through loaded ThingFactory
    // definitions. Supply the same immutable receiving content as C++.
    restored.templates.insert(
        "SavedRepairDock".into(),
        GameLogic::build_template_from_object_definition(
            "SavedRepairDock",
            parser.get_definition("SavedRepairDock").unwrap(),
            None,
        ),
    );
    restored
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "saved-repair-dock.ini".into(),
                source_text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.objects[&dock]
            .repair_dock_health_per_frame
            .to_bits(),
        rate.to_bits()
    );
    for frame in 11..=15 {
        source.frame = frame;
        restored.frame = frame;
        source.update_support_states_for_test(&[master], 1.0 / 30.0);
        restored.update_support_states_for_test(&[master], 1.0 / 30.0);
        let original = &source.objects[&master];
        let resumed = &restored.objects[&master];
        if frame == 12 {
            // Original f32 additions leave the third pulse one ULP below max.
            assert_eq!(original.health.current.to_bits(), 100.0_f32.to_bits() - 1);
            assert_eq!(original.ai_state(), AIState::SeekingRepair);
            assert_eq!(source.objects[&dock].repair_dock_last_id, Some(master));
        } else if frame == 13 {
            assert_eq!(original.health.current, 100.0);
            assert_eq!(original.ai_state(), AIState::SeekingRepair);
            assert_eq!(source.objects[&dock].repair_dock_last_id, Some(master));
        } else if frame == 14 {
            assert_eq!(source.objects[&dock].repair_dock_last_id, None);
        }
        assert_eq!(
            resumed.health.current.to_bits(),
            original.health.current.to_bits()
        );
        assert_eq!(
            resumed.previous_health.to_bits(),
            original.previous_health.to_bits()
        );
        assert_eq!(resumed.last_damage_source, original.last_damage_source);
        assert_eq!(
            resumed.last_healing_timestamp,
            original.last_healing_timestamp
        );
        assert_eq!(resumed.ai_state(), original.ai_state());
        assert_eq!(
            restored.objects[&dock].repair_dock_last_id,
            source.objects[&dock].repair_dock_last_id
        );
        assert_eq!(
            restored.objects[&dock]
                .repair_dock_health_per_frame
                .to_bits(),
            source.objects[&dock].repair_dock_health_per_frame.to_bits()
        );
    }
}

#[test]
fn repair_cache_saved_definition_rebinds_factory_for_future_creations() {
    let (mut source, id) = owner(2.5);
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .restore_repair_dock_state(2.5, Some(ObjectId(90)), 4.0);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiver = GameLogic::new();
    builder
        .restore_from_snapshot(&decoded, &mut receiver)
        .unwrap();
    assert_eq!(
        receiver.templates["RepairCacheOwner"].repair_dock_frames_for_full_heal,
        Some(2.5)
    );
    let created = receiver
        .create_object("RepairCacheOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        receiver.objects[&created]
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(2.5)
    );
    assert_eq!(receiver.objects[&created].repair_dock_last_id, None);
    assert_eq!(receiver.objects[&created].repair_dock_health_per_frame, 0.0);
    assert_eq!(
        receiver.objects[&id].repair_dock_last_id,
        Some(ObjectId(90))
    );
}

#[test]
fn repair_cache_older_snapshot_retains_receiving_definition_with_default_cache() {
    let (source, _) = owner(3.5);
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let offset = capsule_offset(&snapshot.lifecycle_tail);
    let length = u32::from_le_bytes(
        snapshot.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    snapshot.lifecycle_tail.drain(offset..offset + 12 + length);
    let (mut receiving, id) = owner(7.0);
    receiving
        .objects
        .get_mut(&id)
        .unwrap()
        .restore_repair_dock_state(7.0, Some(ObjectId(80)), 9.0);
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving.templates["RepairCacheOwner"].repair_dock_frames_for_full_heal,
        Some(7.0)
    );
    assert_eq!(
        receiving.objects[&id]
            .thing()
            .template
            .repair_dock_frames_for_full_heal,
        Some(7.0)
    );
    assert_eq!(receiving.objects[&id].repair_dock_last_id, None);
    assert_eq!(receiving.objects[&id].repair_dock_health_per_frame, 0.0);
}

#[test]
fn repair_cache_rejects_inconsistent_saved_template_without_receiving_catalog() {
    let (mut source, first) = owner(3.0);
    let second = source
        .create_object("RepairCacheOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    replace_records(
        &mut snapshot,
        &[
            Record {
                object_id: first.0,
                frames: 3.0,
                last_repair: 0,
                health_per_frame: 0.0,
            },
            Record {
                object_id: second.0,
                frames: 4.0,
                last_repair: 0,
                health_per_frame: 0.0,
            },
        ],
    );
    let mut receiver = GameLogic::new();
    receiver.frame = 81;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(receiver.frame, 81);
    assert!(receiver.objects.registered_is_empty());
    assert!(receiver.templates.is_empty());
}
