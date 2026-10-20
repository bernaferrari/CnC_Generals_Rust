//! Actual archive candidate ownership; all particle resources are local owners.
use super::*;
use crate::save_load::{Snapshot, XferLoad, XferSave};
use game_client::effects::particle_system::{Particle, ParticleInfo};
use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
use game_client::system::SubsystemInterface;
use std::sync::Arc;

pub(super) fn local_particle_bytes(name: &str, position: Vec3) -> Vec<u8> {
    let mut manager = local_manager(&[name]);
    let template = manager.find_template(name).unwrap();
    let id = manager.create_particle_system(&template, true).unwrap();
    let system = manager.find_particle_system_mut(id).unwrap();
    system.set_position(position);
    manager
        .admit_particle(
            id,
            Particle::new(
                &ParticleInfo {
                    lifetime: 21,
                    ..Default::default()
                },
                17,
                0,
            ),
        )
        .unwrap();
    manager.capture_xfer_bytes().unwrap()
}

fn local_manager(names: &[&str]) -> ParticleSystemManager {
    let mut manager = ParticleSystemManager::new();
    for name in names {
        let mut template = ParticleSystemTemplate::new((*name).into());
        template.info_mut().system_lifetime = 1000;
        manager.register_template(Arc::new(template));
    }
    manager
}
fn archive(companion: Option<Vec<u8>>) -> Vec<u8> {
    let mut snapshot = WorldSnapshot::default();
    snapshot.particle_system_companion = companion;
    SaveFileManager::write_common_sav_chunks(
        &snapshot,
        &fixture_save_info(),
        CPP_GAME_SINGLE_PLAYER,
    )
    .unwrap()
}
fn parsed(bytes: &[u8]) -> WorldSnapshot {
    SaveFileManager::read_common_sav_chunks(bytes, Path::new(""))
        .unwrap()
        .snapshot
}
fn repack(blocks: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, payload) in blocks {
        bytes.push(u8::try_from(name.len()).unwrap());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&payload);
    }
    bytes.push(SAVE_FILE_EOF.len() as u8);
    bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());
    bytes
}
fn assert_loaded(manager: &mut ParticleSystemManager, snapshot: &WorldSnapshot, expected: Vec3) {
    let receipt = manager
        .restore_xfer_bytes(snapshot.particle_system_companion.as_deref().unwrap())
        .unwrap()
        .unwrap();
    assert!(receipt.belongs_to(manager));
    assert!(receipt.contains_system(1));
    assert_eq!(manager.active_system_count(), 1);
    let system = manager.find_particle_system(1).unwrap();
    assert_eq!(system.position(), expected);
    assert_eq!(system.particle_count(), 1);
    assert_eq!(system.particles()[0].lifetime, 21);
}

#[test]
fn particle_candidate_archives_interleave_without_stealing_receiving_payloads() {
    let a_position = Vec3::new(12.0, 22.0, 32.0);
    let b_position = Vec3::new(15.0, 25.0, 35.0);
    let a_bytes = local_particle_bytes("ParticleCandidateA", a_position);
    let b_bytes = local_particle_bytes("ParticleCandidateB", b_position);
    let a = parsed(&archive(Some(a_bytes.clone())));
    let b = parsed(&archive(Some(b_bytes.clone())));
    assert_eq!(
        a.particle_system_companion.as_deref(),
        Some(a_bytes.as_slice())
    );
    assert_eq!(
        b.particle_system_companion.as_deref(),
        Some(b_bytes.as_slice())
    );
    let mut receiving_a = local_manager(&["ParticleCandidateA"]);
    let mut receiving_b = local_manager(&["ParticleCandidateB"]);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_loaded(&mut receiving_b, &b, b_position);
    receiving_b.reset().unwrap();
    assert_eq!(receiving_b.active_system_count(), 0);
    drop(receiving_b);
    assert_loaded(&mut receiving_a, &a, a_position);
    assert_eq!(
        b.particle_system_companion.as_deref(),
        Some(b_bytes.as_slice())
    );
}

#[test]
fn particle_candidate_resave_uses_exact_owned_bytes_and_missing_stays_missing() {
    let payload = local_particle_bytes("OwnedResave", Vec3::new(7.0, 8.0, 9.0));
    let a = parsed(&archive(Some(payload.clone())));
    let absent = parsed(&archive(None));
    let _b = parsed(&archive(Some(local_particle_bytes(
        "PeerResave",
        Vec3::splat(99.0),
    ))));
    for (snapshot, expected) in [(&a, payload.as_slice()), (&absent, &[1][..])] {
        let bytes = SaveFileManager::write_common_sav_chunks(
            snapshot,
            &fixture_save_info(),
            CPP_GAME_SINGLE_PLAYER,
        )
        .unwrap();
        let blocks = walk_named_chunks(&bytes).unwrap();
        let actual = &blocks
            .iter()
            .find(|(name, _)| name == CHUNK_PARTICLE_SYSTEM)
            .unwrap()
            .1;
        assert_eq!(actual, expected);
    }
}

#[test]
fn particle_candidate_missing_empty_and_failed_decode_do_not_contaminate_prior_candidate() {
    let payload = local_particle_bytes("CandidateSurvives", Vec3::new(3.0, 4.0, 5.0));
    let a = parsed(&archive(Some(payload.clone())));
    let valid_blocks = walk_named_chunks(&archive(Some(local_particle_bytes(
        "FailedCandidate",
        Vec3::ZERO,
    ))))
    .unwrap();
    let mut corrupt = valid_blocks.clone();
    corrupt
        .iter_mut()
        .find(|(name, _)| name == CHUNK_GAME_LOGIC)
        .unwrap()
        .1 = vec![0xFF];
    assert!(SaveFileManager::read_common_sav_chunks(&repack(corrupt), Path::new("")).is_err());
    for mode in 0..3 {
        let mut blocks = valid_blocks.clone();
        if mode == 0 {
            blocks.retain(|(name, _)| name != CHUNK_PARTICLE_SYSTEM);
        } else {
            blocks
                .iter_mut()
                .find(|(name, _)| name == CHUNK_PARTICLE_SYSTEM)
                .unwrap()
                .1 = if mode == 1 { Vec::new() } else { vec![1] };
        }
        let absent = parsed(&repack(blocks));
        assert!(absent.particle_system_companion.is_none());
    }
    assert_eq!(
        a.particle_system_companion.as_deref(),
        Some(payload.as_slice())
    );
    let mut manager = local_manager(&["CandidateSurvives"]);
    assert_loaded(&mut manager, &a, Vec3::new(3.0, 4.0, 5.0));
}

#[test]
fn particle_candidate_companion_is_skipped_by_serde_and_cleared_on_direct_xfer_reuse() {
    let mut snapshot = WorldSnapshot::default();
    let without = bincode_legacy::serialize(&snapshot).unwrap();
    snapshot.particle_system_companion =
        Some(local_particle_bytes("SkippedNativeCompanion", Vec3::ZERO));
    assert_eq!(bincode_legacy::serialize(&snapshot).unwrap(), without);
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&without).unwrap();
    assert!(decoded.particle_system_companion.is_none());
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    snapshot
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    assert!(snapshot.particle_system_companion.is_none());
}

// This whole module uses only packet407 APIs. Append to save_file/tests.rs
// in an OLD source control and to the NEW candidate test module unchanged.
#[cfg(feature = "game_client")]
mod particle_companion_runtime_control {
    use super::*;
    use crate::game_logic::combat_particles::with_client_particle_manager;
    use crate::save_load::SnapshotBuilder;
    use game_client::effects::{ParticleSystemManager, ParticleSystemTemplate};
    use game_client::system::SubsystemInterface;
    use std::sync::Arc;

    fn native_payload(name: &str, position: Vec3) -> Vec<u8> {
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new(name.into());
        template.info_mut().system_lifetime = 1000;
        let template = Arc::new(template);
        manager.register_template(template.clone());
        let id = manager.create_particle_system(&template, true).unwrap();
        assert_eq!(id, 1);
        manager
            .find_particle_system_mut(id)
            .unwrap()
            .set_position(position);
        manager.capture_xfer_bytes().unwrap()
    }

    fn archive_with_raw_particle_chunk(payload: Vec<u8>) -> Vec<u8> {
        let bytes = SaveFileManager::write_common_sav_chunks(
            &WorldSnapshot::default(),
            &fixture_save_info(),
            CPP_GAME_SINGLE_PLAYER,
        )
        .unwrap();
        let mut blocks = walk_named_chunks(&bytes).unwrap();
        blocks
            .iter_mut()
            .find(|(name, _)| name == CHUNK_PARTICLE_SYSTEM)
            .unwrap()
            .1 = payload;
        let mut archive = Vec::new();
        for (name, payload) in blocks {
            archive.push(u8::try_from(name.len()).unwrap());
            archive.extend_from_slice(name.as_bytes());
            archive.extend_from_slice(&i32::try_from(payload.len()).unwrap().to_le_bytes());
            archive.extend_from_slice(&payload);
        }
        archive.push(SAVE_FILE_EOF.len() as u8);
        archive.extend_from_slice(SAVE_FILE_EOF.as_bytes());
        archive
    }

    #[test]
    fn particle_companion_runtime_interleaved_archive_restore_uses_a_not_pending_b() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "particle_companion_runtime_interleaved_archive_restore_uses_a_not_pending_b",
            || {
                let position_a = Vec3::new(12.0, 23.0, 34.0);
                let position_b = Vec3::new(56.0, 67.0, 78.0);
                let payload_a = native_payload("OldCompatibleParticleA", position_a);
                let payload_b = native_payload("OldCompatibleParticleB", position_b);
                with_client_particle_manager(|manager| {
                    manager.reset().unwrap();
                    for name in ["OldCompatibleParticleA", "OldCompatibleParticleB"] {
                        let mut template = ParticleSystemTemplate::new(name.into());
                        template.info_mut().system_lifetime = 1000;
                        manager.register_template(Arc::new(template));
                    }
                })
                .unwrap();
                let archive_a = archive_with_raw_particle_chunk(payload_a);
                let archive_b = archive_with_raw_particle_chunk(payload_b);
                let crate::save_load::DecodedGameSave {
                    snapshot: candidate_a,
                    ..
                } = SaveFileManager::read_common_sav_chunks(&archive_a, Path::new("")).unwrap();
                let crate::save_load::DecodedGameSave {
                    snapshot: _candidate_b,
                    ..
                } = SaveFileManager::read_common_sav_chunks(&archive_b, Path::new("")).unwrap();
                // OLD407's shared pending slot now contains B. Restoring A must use
                // A's actual native archive, irrespective of later candidate parsing.
                let mut receiving_world = GameLogic::new();
                SnapshotBuilder::new()
                    .restore_from_snapshot(&candidate_a, &mut receiving_world)
                    .unwrap();
                with_client_particle_manager(|manager| {
                    assert_eq!(manager.active_system_count(), 1);
                    let actual = manager.find_particle_system(1).unwrap();
                    assert_eq!(
                        actual.position(),
                        position_a,
                        "restoring A consumed B's pending native particle payload"
                    );
                    assert_eq!(actual.template().name(), "OldCompatibleParticleA");
                    manager.reset().unwrap();
                })
                .unwrap();
            },
        );
    }
}
