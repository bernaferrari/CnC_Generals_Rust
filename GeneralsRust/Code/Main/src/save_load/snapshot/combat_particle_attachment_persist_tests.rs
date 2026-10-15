use super::super::Snapshot;
use super::*;
use crate::game_logic::{CombatParticleKind, Team, ThingTemplate};
use crate::save_load::SnapshotBuilder;

fn fixture() -> (GameLogic, u32, CombatParticleAttachment) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("AttachmentOwner");
    template.set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let owner = world
        .create_object("AttachmentOwner", Team::USA, Vec3::new(11.0, 12.0, 13.0))
        .unwrap();
    let id = world.combat_particles_mut().spawn(
        CombatParticleKind::DeathExplosion,
        Vec3::new(91.0, 92.0, 93.0),
        7,
        Some(owner),
        None,
    );
    let attachment = CombatParticleAttachment {
        owner,
        local_offset: Vec3::new(2.5, -3.25, 8.0),
        lifetime: Some(45),
        local_z_rotation: 0.75,
    };
    world.combat_particles_mut().get_mut(id).unwrap().attachment = Some(attachment.clone());
    (world, id, attachment)
}

fn copied_snapshot(snapshot: &WorldSnapshot) -> WorldSnapshot {
    bincode_legacy::deserialize(&bincode_legacy::serialize(snapshot).unwrap()).unwrap()
}

#[test]
fn combat_particle_attachment_native_and_serde_round_trip_preserve_exact_descriptor() {
    let (world, id, expected) = fixture();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    assert!(
        decoded
            .combat_particles
            .systems
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .attachment
            .is_none(),
        "nested positional entry must retain historical shape"
    );
    assert_eq!(decode_and_validate(&decoded).unwrap()[&id], expected);
    let mut restored = GameLogic::new();
    restored.templates = world.templates.clone();
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let entry = restored.combat_particles().get(id).unwrap();
    assert_eq!(entry.attachment, Some(expected.clone()));
    assert_eq!(
        entry.position,
        Vec3::new(91.0, 92.0, 93.0),
        "world XYZ is independent from the saved local attachment"
    );
    #[cfg(feature = "game_client")]
    {
        use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
        use std::sync::Arc;
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new(entry.template_name.clone());
        template.info_mut().system_lifetime = 1000;
        manager.register_template(Arc::new(template));
        assert_eq!(
            restored
                .combat_particles_mut()
                .backfill_client_systems(&mut manager),
            1
        );
        let client_id = restored
            .combat_particles()
            .get(id)
            .unwrap()
            .client_system_id
            .unwrap();
        let actual = manager.find_particle_system(client_id).unwrap();
        assert_eq!(actual.attached_object(), Some(expected.owner.0));
        assert_eq!(
            actual.position(),
            Vec3::new(
                expected.local_offset.x,
                expected.local_offset.z,
                expected.local_offset.y
            )
        );
        assert_eq!(
            actual.local_transform(),
            glam::Mat3::from_rotation_z(expected.local_z_rotation)
        );
        assert_eq!(
            restored
                .combat_particles_mut()
                .backfill_client_systems(&mut manager),
            0
        );
        assert_eq!(manager.active_system_count(), 1);
        let actual = manager.find_particle_system_mut(client_id).unwrap();
        for frame in 1..expected.lifetime.unwrap() {
            actual.finish_frame_integrate(frame);
        }
        assert!(
            !actual.should_remove(),
            "explicit attachment lifetime has not elapsed"
        );
        actual.finish_frame_integrate(expected.lifetime.unwrap());
        assert!(
            actual.should_remove(),
            "saved explicit lifetime overrides the thousand-frame template"
        );
    }
}

#[test]
fn combat_particle_attachment_absent_historical_capsule_keeps_unattached_semantics() {
    let (world, _, _) = fixture();
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    let (start, footer) = framing(&snapshot.lifecycle_tail).unwrap().unwrap();
    snapshot.lifecycle_tail.drain(start..footer + 12);
    assert!(decode_and_validate(&snapshot).unwrap().is_empty());
    assert_eq!(
        super::super::construction_completion_persist::suffix_start(&snapshot.lifecycle_tail)
            .unwrap(),
        super::super::construction_completion_persist::own_suffix_start(&snapshot.lifecycle_tail)
            .unwrap()
    );
}

#[test]
fn combat_particle_attachment_rejects_malformed_keyed_descriptors_before_restore() {
    let (world, _, _) = fixture();
    let baseline = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    let (start, _) = framing(&baseline.lifecycle_tail).unwrap().unwrap();
    // Host id, owner id, nonfinite offset, invalid optional lifetime, nonfinite rotation.
    for (offset, value) in [
        (4, u32::MAX),
        (8, u32::MAX),
        (12, f32::NAN.to_bits()),
        (24, 2),
        (32, f32::INFINITY.to_bits()),
    ] {
        let mut snapshot = copied_snapshot(&baseline);
        snapshot.lifecycle_tail[start + offset..start + offset + 4]
            .copy_from_slice(&value.to_le_bytes());
        assert!(
            decode_and_validate(&snapshot).is_err(),
            "accepted offset {offset}"
        );
    }
    let mut duplicate_entry = copied_snapshot(&baseline);
    duplicate_entry
        .combat_particles
        .systems
        .push(duplicate_entry.combat_particles.systems[0].clone());
    assert!(decode_and_validate(&duplicate_entry).is_err());
}

#[test]
fn combat_particle_attachment_rejects_duplicate_records_and_footer_corruption() {
    let (world, _, _) = fixture();
    let baseline = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    let (start, footer) = framing(&baseline.lifecycle_tail).unwrap().unwrap();
    let mut duplicate = copied_snapshot(&baseline);
    let record = duplicate.lifecycle_tail[start + 4..footer].to_vec();
    duplicate.lifecycle_tail.splice(footer..footer, record);
    duplicate.lifecycle_tail[start..start + 4].copy_from_slice(&2u32.to_le_bytes());
    duplicate.lifecycle_tail[footer + RECORD_BYTES + 4..footer + RECORD_BYTES + 8]
        .copy_from_slice(&(4u32 + 2 * RECORD_BYTES as u32).to_le_bytes());
    assert!(decode_and_validate(&duplicate).is_err());
    for (offset, value) in [(footer, 99u32), (footer + 4, u32::MAX)] {
        let mut malformed = copied_snapshot(&baseline);
        malformed.lifecycle_tail[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(decode_and_validate(&malformed).is_err());
    }
}

#[test]
fn combat_particle_attachment_positional_xfer_preserves_tail_descriptor_not_runtime_handle() {
    use crate::save_load::{XferLoad, XferSave};
    use std::io::Cursor;
    let (world, id, expected) = fixture();
    let mut snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut restored = WorldSnapshot::default();
    restored
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    assert_eq!(decode_and_validate(&restored).unwrap()[&id], expected);
    assert!(
        restored
            .combat_particles
            .systems
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .attachment
            .is_none()
    );
}
